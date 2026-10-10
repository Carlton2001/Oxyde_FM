//! Fresh, inert fixtures. Run after `cargo build --bin oxyde`; never starts GPUI.
#[cfg(target_os = "windows")]
#[path = "../tests/support/iso.rs"]
mod iso;

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use oxyde_lib::{
        systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
        utils::archive_io_protocol::with_worker_executable_for_tests,
    };
    use std::{
        fs,
        io::Write,
        path::Path,
        sync::{Arc, Mutex},
    };
    fn operation(kind: FileOpType) -> Arc<Mutex<FileOperation>> {
        let mut op = FileOperation::new(kind, vec![], None, None);
        op.compression_quality = "fast".into();
        Arc::new(Mutex::new(op))
    }
    fn zip(path: &Path, names: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = zip::ZipWriter::new(fs::File::create_new(path)?);
        for name in names {
            writer.start_file(*name, zip::write::SimpleFileOptions::default())?;
            writer.write_all(b"Fixture inerte Oxyde\n")?;
        }
        writer.finish()?;
        Ok(())
    }
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let executable = project.join("target/debug/oxyde.exe");
    if !executable.is_file() {
        return Err("Build target/debug/oxyde.exe first".into());
    }
    let root = project.join("target").join(format!(
        "security-visual-isolation-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root)?;
    let root = root.canonicalize()?;
    for folder in [
        "archives",
        "sources/école_日本語/vide",
        "modification",
        "transfert",
        "destinations/extraction",
        "destinations/export",
        "destinations/transfert",
        "references",
        "aperçus",
        "cache-indisponible",
    ] {
        fs::create_dir_all(root.join(folder))?;
    }
    let source = root.join("sources/école_日本語");
    let note = "Oxyde — école 日本語\nContenu de référence pour ouverture, export et transfert.\n";
    fs::write(source.join("note.txt"), note)?;
    fs::write(source.join("vide.txt"), b"")?;
    fs::write(root.join("cache-indisponible/oxyde-archive-open"), "Obstruction inerte du cache de cette fixture.\n")?;
    fs::write(root.join("destinations/export/note.txt"), "TÉMOIN À CONSERVER\n")?;
    let mut seed = 0x1a37u32;
    let data: Vec<u8> = (0..32768)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        })
        .collect();
    fs::write(source.join("donnees.bin"), &data)?;
    let result: Result<(), Box<dyn std::error::Error>> =
        with_worker_executable_for_tests(&executable, || {
            for (number, extension) in [(1, "zip"), (2, "7z"), (3, "tar"), (7, "tar.zst")] {
                let archive = root.join(format!("archives/{number:02}-normal.{extension}"));
                FileOperationManager::perform_compress(
                    &operation(FileOpType::Compress),
                    vec![source.clone()],
                    Some(archive.clone()),
                )?;
                fs::copy(
                    archive,
                    root.join(format!("modification/travail.{extension}")),
                )?;
            }
            let tar = fs::read(root.join("archives/03-normal.tar"))?;
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            gz.write_all(&tar)?;
            let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
            xz.write_all(&tar)?;
            let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
            bz.write_all(&tar)?;
            for (name, bytes) in [
                ("04-normal.tar.gz", gz.finish()?),
                ("05-normal.tar.xz", xz.finish()?),
                ("06-normal.tar.bz2", bz.finish()?),
            ] {
                fs::write(root.join("archives").join(name), bytes)?;
            }
            iso::iso_fixture(&root.join("archives/08-normal.iso"));
            fs::copy(
                root.join("archives/08-normal.iso"),
                root.join("archives/09-normal.img"),
            )?;
            for extension in ["zip", "7z"] {
                FileOperationManager::perform_compress(
                    &operation(FileOpType::Compress),
                    vec![source.join("note.txt")],
                    Some(root.join(format!("transfert/source.{extension}"))),
                )?;
                FileOperationManager::perform_compress(
                    &operation(FileOpType::Compress),
                    vec![source.join("vide")],
                    Some(root.join(format!("transfert/cible.{extension}"))),
                )?;
            }
            for item in fs::read_dir(root.join("archives"))? {
                let archive = item?.path();
                let target = root.join("references").join(archive.file_name().unwrap());
                FileOperationManager::perform_extract(
                    &operation(FileOpType::Extract),
                    vec![archive.clone()],
                    Some(target.clone()),
                )?;
                if archive
                    .extension()
                    .is_some_and(|ext| ext == "iso" || ext == "img")
                {
                    if fs::read(target.join("DOCS/README.TXT"))? != b"ISO content" {
                        return Err("ISO reference mismatch".into());
                    }
                } else {
                    if fs::read(target.join("école_日本語/note.txt"))? != note.as_bytes()
                        || fs::read(target.join("école_日本語/donnees.bin"))? != data
                        || !target.join("école_日本語/vide").is_dir()
                        || fs::metadata(target.join("école_日本語/vide.txt"))?.len() != 0
                    {
                        return Err("Archive reference mismatch".into());
                    }
                }
            }
            Ok(())
        });
    result?;
    fs::write(
        root.join("archives/10-corrompu.zip"),
        b"Oxyde: ZIP volontairement tronque",
    )?;
    zip(
        &root.join("archives/11-chemin-refuse.zip"),
        &["note.txt", "../sortie-interdite.txt"],
    )?;
    let deep = format!("{}note.txt", "d/".repeat(65));
    zip(&root.join("archives/12-profondeur-refusee.zip"), &[&deep])?;
    image::RgbaImage::from_pixel(32, 32, image::Rgba([40, 130, 220, 255]))
        .save(root.join("aperçus/image.png"))?;
    // Four objects, exact offsets: small PDF without scripts, attachments or links.
    let content = b"1 0 0 rg 0 0 72 144 re f\n";
    let objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 144] /Resources << >> /Contents 4 0 R >>"
            .to_vec(),
        [
            format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
            content,
            b"endstream",
        ]
        .concat(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n", index + 1)?;
        pdf.extend(object);
        pdf.extend(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        writeln!(pdf, "{offset:010} 00000 n ")?;
    }
    write!(
        pdf,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )?;
    fs::write(root.join("aperçus/document.pdf"), pdf)?;
    fs::write(root.join("LISEZ-MOI.txt"), "Jeu neuf, inerte et indépendant. Archives normales vérifiées par extraction via le worker natif. Les références sont conservées pour comparaison. Modifier les copies dans modification, utiliser transfert/source et cible pour copie/déplacement, et destinations pour les sorties. Le ZIP chemin-refuse ne doit jamais créer sortie-interdite.txt. Protocole détaillé : security/VERIFICATION_VISUELLE.md. Aucun lancement d'interface ni application associée par ce générateur.\n")?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(&root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        total += bytes.len() as u64;
        files.push(serde_json::json!({"path": entry.path().strip_prefix(&root)?.to_string_lossy(), "bytes": bytes.len(), "blake3": blake3::hash(&bytes).to_hex().to_string()}));
    }
    files.sort_by_key(|value| value["path"].as_str().unwrap().to_owned());
    if total > 1024 * 1024 {
        return Err("Fixtures exceed 1 MiB".into());
    }
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"files": files, "total_bytes_without_manifest": total, "native_roundtrip": true}),
        )?,
    )?;
    println!(
        "{}\n{} fichiers, {} octets hors manifeste ; extraction native vérifiée.",
        root.display(),
        files.len(),
        total
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("These fixtures validate the Windows native worker; run on Windows.");
}
