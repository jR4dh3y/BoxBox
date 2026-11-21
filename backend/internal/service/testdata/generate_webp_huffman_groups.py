"""Generate ordinary and adversarial WebP fixtures for thumbnail memory tests.

Adversarial streams reference 2600 full Huffman groups with an 11-bit color cache.
Run this file to regenerate the lossless, opaque, and compressed-alpha fixtures.
"""

import base64
import gzip
import struct
from pathlib import Path


class Bits:
    def __init__(self):
        self.data = bytearray()
        self.value = 0
        self.count = 0

    def put(self, value, count):
        self.value |= value << self.count
        self.count += count
        while self.count >= 8:
            self.data.append(self.value & 255)
            self.value >>= 8
            self.count -= 8

    def code(self, value, count):
        for bit in range(count - 1, -1, -1):
            self.put((value >> bit) & 1, 1)

    def finish(self):
        if self.count:
            self.data.append(self.value)
        return bytes(self.data)


def tree(bits, size):
    short = size.bit_length() - 1
    shorter = (1 << (short + 1)) - size
    lengths = [short] * shorter + [short + 1] * (size - shorter)
    bits.put(0, 1)  # Normal Huffman tree.
    bits.put(15, 4)  # All 19 code-length alphabet entries.
    for symbol in [17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]:
        bits.put(4 if symbol < 16 else 0, 3)
    bits.put(0, 1)  # Encode every symbol length.
    for length in lengths:
        bits.code(length, 4)
    return [(symbol if symbol < shorter else symbol + shorter, length)
            for symbol, length in enumerate(lengths)]


def webp(chunks):
    form = b'WEBP'
    for tag, data in chunks:
        form += tag + struct.pack('<I', len(data)) + data + b'\0' * (len(data) & 1)
    return b'RIFF' + struct.pack('<I', len(form)) + form


def compressed_fixture(name, encoded):
    destination = Path(__file__).with_name(name)
    with destination.open('wb') as output:
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0) as compressed:
            compressed.write(encoded)


def ordinary_lossless(width, height):
    bits = Bits()
    bits.put(0x2f, 8)
    bits.put(width - 1, 14)
    bits.put(height - 1, 14)
    bits.put(0, 4)
    bits.put(0, 3)  # No transforms, color cache, or meta-image.
    for symbol in [0, 0, 0, 255, 0]:
        bits.put(1, 1)
        bits.put(0, 1)  # A single-symbol Huffman tree.
        bits.put(int(symbol > 1), 1)
        bits.put(symbol, 8 if symbol > 1 else 1)
    return bits.finish() + b'\0' * 8


def many_group_lossless(width, height, tile_bits):
    bits = Bits()
    bits.put(0x2f, 8)
    bits.put(width - 1, 14)
    bits.put(height - 1, 14)
    bits.put(0, 4)
    bits.put(0, 1)  # No transforms.
    bits.put(1, 1)
    bits.put(11, 4)  # 2048-entry color cache.
    bits.put(1, 1)
    bits.put(tile_bits - 2, 3)
    bits.put(0, 1)  # Meta-image has no color cache.
    tables = [tree(bits, size) for size in [280, 256, 256, 256, 40]]
    tiles = ((width + (1 << tile_bits) - 1) >> tile_bits) * ((height + (1 << tile_bits) - 1) >> tile_bits)
    for tile in range(tiles):
        group = tile % 2600
        for channel, symbol in enumerate([group & 255, group >> 8, 0, 255]):
            bits.code(*tables[channel][symbol])
    for _ in range(2600):
        for size in [2328, 256, 256, 256, 40]:
            tree(bits, size)
    bits.put(0, 11 + 8 + 8)
    bits.put(255, 8)  # First pixel is opaque black.
    remaining = width * height - 1
    while remaining:
        length = min(4096, remaining)
        for symbol in range(24):
            extra = 0 if symbol < 4 else (symbol - 2) // 2
            base = symbol + 1 if symbol < 4 else ((2 + (symbol & 1)) << extra) + 1
            if base <= length < base + (1 << extra):
                bits.code(256 + symbol, 11)
                bits.put(length - base, extra)
                break
        bits.code(1, 5)  # Distance code 2 repeats the immediately preceding pixel.
        remaining -= length
    return bits.finish() + b'\0' * 8


data = many_group_lossless(208, 200, 2)
compressed_fixture('webp-huffman-groups.webp.gz', webp([(b'VP8L', data)]))
compressed_fixture('webp-large-huffman-groups.webp.gz', webp([(b'VP8L', many_group_lossless(4096, 3072, 6))]))

# Solid 208x200 VP8 frame generated with ImageMagick's lossy WebP encoder.
opaque = base64.b64decode('kAwAnQEq0ADIAD6RSKFNJaQjIiBIALASCWlu4XaxHtwAADDGpNdtovMNdtouEGQQ1VJrttFwgyCGqpNdtouEGQQ1VJrttFwgyCGqpNdtouEGQQ1VJrttFwgyCGqpNdtouEGQQ1VJrttFwgxyAAD+/klf/8kj1cx0H/+tTIx8kIN34HBwAAAAAAAA')
Path(__file__).with_name('webp-opaque.webp').write_bytes(webp([(b'VP8 ', opaque)]))
canvas = bytes([16, 0, 0, 0]) + (207).to_bytes(3, 'little') + (199).to_bytes(3, 'little')
compressed_fixture('webp-alpha-huffman-groups.webp.gz', webp([
    (b'VP8X', canvas), (b'ALPH', b'\x01' + data[5:]), (b'VP8 ', opaque),
]))
for width, height in [(1024, 1024), (2048, 1536), (4096, 3072)]:
    Path(__file__).with_name(f'webp-{width}x{height}.webp').write_bytes(
        webp([(b'VP8L', ordinary_lossless(width, height))]))
