"""Generate a valid 208x200 VP8L fixture with 2600 referenced, full Huffman groups.

The 11-bit color cache maximizes the green alphabet in x/image v0.47.0.
Run this file to regenerate webp-huffman-groups.webp.gz beside it.
"""

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


bits = Bits()
bits.put(0x2f, 8)
bits.put(207, 14)
bits.put(199, 14)
bits.put(0, 4)
bits.put(0, 1)  # No transforms.
bits.put(1, 1)
bits.put(11, 4)  # 2048-entry color cache.
bits.put(1, 1)
bits.put(0, 3)  # 4x4 meta-image tiles.
bits.put(0, 1)  # Meta-image has no color cache.
tables = [tree(bits, size) for size in [280, 256, 256, 256, 40]]
for group in range(2600):
    for channel, symbol in enumerate([group & 255, group >> 8, 0, 255]):
        bits.code(*tables[channel][symbol])
for _ in range(2600):
    for size in [2328, 256, 256, 256, 40]:
        tree(bits, size)
for _ in range(208 * 200):
    bits.put(0, 11 + 8 + 8)
    bits.put(255, 8)
data = bits.finish() + b'\0' * 8
chunk = b'VP8L' + struct.pack('<I', len(data)) + data + b'\0' * (len(data) & 1)
form = b'WEBP' + chunk
encoded = b'RIFF' + struct.pack('<I', len(form)) + form
destination = Path(__file__).with_name('webp-huffman-groups.webp.gz')
with destination.open('wb') as output:
    with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0) as compressed:
        compressed.write(encoded)
