#![cfg(windows)]

use std::fs;

// The PDF has a red rectangle on its first page. A file icon would leave no red pixels.
#[test]
fn pdf_thumbnail_contains_the_first_page() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("page.pdf");
    let content = "1 0 0 rg 10 10 80 80 re f\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> /Contents 4 0 R >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut entries = String::from("0000000000 65535 f \n");
    for (index, object) in objects.iter().enumerate() {
        entries.push_str(&format!("{:010} 00000 n \n", pdf.len()));
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 5\n{entries}trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    fs::write(&path, pdf).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_boxbox-gpui"))
        .args(["--thumbnail-worker", "96", "96"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let image = image::load_from_memory(&output.stdout).unwrap().into_rgb8();
    let center = image.get_pixel(image.width() / 2, image.height() / 2);
    assert!(
        center[0] > 230 && center[1] < 30 && center[2] < 30,
        "{center:?}"
    );
}

#[test]
#[ignore = "requires the Windows Media Foundation feature"]
fn windows_audio_without_cover_art_has_a_waveform() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("waveform.wav");
    let rate = 16_000u32;
    let mut samples = Vec::new();
    for sample in 0..rate {
        let value: i16 = if sample % 32 < 16 { 14_000 } else { -14_000 };
        samples.extend(value.to_le_bytes());
    }
    let len = samples.len() as u32;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + len).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes()); // PCM
    wav.extend(1u16.to_le_bytes()); // mono
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(len.to_le_bytes());
    wav.extend(samples);
    fs::write(&path, wav).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_boxbox-gpui"))
        .args(["--thumbnail-worker", "96", "96"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let image = image::load_from_memory(&output.stdout).unwrap().into_rgb8();
    assert!(image.pixels().any(|pixel| pixel.0 == [96, 165, 250]));
}
