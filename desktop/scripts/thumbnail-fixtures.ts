import { writeFile } from "node:fs/promises";
import { join } from "node:path";

export async function createThumbnailFixtures(root: string): Promise<void> {
  const ffmpeg = Bun.which("ffmpeg");
  if (!ffmpeg || !Bun.which("pdftoppm"))
    throw new Error("Media thumbnail checks require FFmpeg and poppler-utils");
  const generate = async (args: string[]) => {
    const child = Bun.spawn(
      [ffmpeg, "-nostdin", "-hide_banner", "-loglevel", "error", "-y", ...args],
      { stdout: "ignore", stderr: "pipe" },
    );
    const error = await new Response(child.stderr).text();
    if ((await child.exited) !== 0)
      throw new Error(`Fixture generation failed: ${error}`);
  };
  await generate([
    "-f",
    "lavfi",
    "-i",
    "color=c=red:s=640x360:r=10:d=0.5",
    "-c:v",
    "mpeg4",
    "-q:v",
    "3",
    join(root, "clip.mp4"),
  ]);
  await generate([
    "-f",
    "lavfi",
    "-i",
    "color=c=blue:s=320x200",
    "-frames:v",
    "1",
    "-threads",
    "1",
    join(root, "cover.png"),
  ]);
  await generate([
    "-i",
    join(root, "cover.png"),
    "-frames:v",
    "1",
    "-threads",
    "1",
    join(root, "photo.bmp"),
  ]);
  await generate([
    "-f",
    "lavfi",
    "-i",
    "sine=frequency=440:duration=1",
    join(root, "tone.wav"),
  ]);
  await generate([
    "-i",
    join(root, "tone.wav"),
    "-i",
    join(root, "cover.png"),
    "-map",
    "0:a",
    "-map",
    "1:v",
    "-c:a",
    "libmp3lame",
    "-c:v",
    "png",
    "-disposition:v",
    "attached_pic",
    "-id3v2_version",
    "3",
    join(root, "album.mp3"),
  ]);
  const stream = "0 0 1 rg 0 0 320 200 re f\n";
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 200] /Contents 4 0 R /Resources << >> >>",
    `<< /Length ${Buffer.byteLength(stream)} >>\nstream\n${stream}endstream`,
  ];
  let pdf = "%PDF-1.4\n";
  const offsets = [0];
  for (const [index, object] of objects.entries()) {
    offsets.push(Buffer.byteLength(pdf));
    pdf += `${index + 1} 0 obj\n${object}\nendobj\n`;
  }
  const xref = Buffer.byteLength(pdf);
  pdf += `xref\n0 5\n0000000000 65535 f \n${offsets
    .slice(1)
    .map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`)
    .join("")}`;
  pdf += `trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  await writeFile(join(root, "document.pdf"), pdf);
  await writeFile(join(root, "broken.mp4"), "not a video");
  await writeFile(join(root, "notes.txt"), "no thumbnail");
}
