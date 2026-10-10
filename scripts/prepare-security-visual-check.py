"""Small, inert fixtures for Oxyde's manual security UI check (stdlib only).

Generation creates a new directory below target; no existing file is overwritten.
The depth ZIP contains one byte; no payload is executed or extracted by the script.
The optional launch starts the built Oxyde with a private child-only TEMP.
"""
from pathlib import Path
import argparse
import binascii
import json
import struct
import subprocess
import os
import tempfile
import io
import tarfile
import zipfile
import zlib


def png(width, height, color):
    def chunk(kind, payload):
        return (struct.pack(">I", len(payload)) + kind + payload
                + struct.pack(">I", binascii.crc32(kind + payload)))

    rows = (b"\0" + bytes(color) * width) * height
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def gif(frame_count):
    # Reset the LZW dictionary for each literal: always nine-bit codes,
    # independent of dictionary growth. Alternate unmistakable red/blue frames.
    data = bytearray(b"GIF89a" + struct.pack("<HHBBB", 16, 16, 0xF7, 0, 0))
    data.extend(bytes(channel for index in range(256)
                      for channel in ((255, 0, 0) if index % 2 == 0 else (0, 0, 255))))
    data.extend(b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00")
    for frame in range(frame_count):
        data.extend(b"\x21\xf9\x04\x08\x19\x00\x00\x00")
        data.extend(b"\x2c" + struct.pack("<HHHHB", 0, 0, 16, 16, 0))
        codes = [code for _ in range(256) for code in (256, frame % 256)] + [257]
        packed = bytearray()
        bits, count = 0, 0
        for code in codes:
            bits |= code << count
            count += 9
            while count >= 8:
                packed.append(bits & 255)
                bits >>= 8
                count -= 8
        if count:
            packed.append(bits & 255)
        data.append(8)
        for offset in range(0, len(packed), 255):
            block = packed[offset:offset + 255]
            data.append(len(block))
            data.extend(block)
        data.append(0)
    data.append(0x3B)
    return bytes(data)


def pdf():
    content = b"1 0 0 rg 0 0 72 144 re f\n"
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",
               b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 144] /Resources << >> /Contents 4 0 R >>",
               b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"endstream"]
    data, offsets = bytearray(b"%PDF-1.4\n"), []
    for index, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(str(index).encode() + b" 0 obj\n" + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(b"xref\n0 5\n0000000000 65535 f \n")
    for offset in offsets:
        data.extend(f"{offset:010} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


def video():
    def chunk(kind, data):
        return kind + struct.pack("<I", len(data)) + data + (b"\0" if len(data) % 2 else b"")

    def numbers(*values):
        return struct.pack("<" + "I" * len(values), *values)

    stream = b"vidsDIB\0" + numbers(0, 0, 0, 1, 1, 0, 2, 0, 0xFFFFFFFF, 0) + struct.pack("<4H", 0, 0, 16, 16)
    bitmap = numbers(40, 16, 16) + struct.pack("<HH", 1, 32) + numbers(0, 1024, 0, 0, 0, 0)
    strl = b"strl" + chunk(b"strh", stream) + chunk(b"strf", bitmap)
    header = b"hdrl" + chunk(b"avih", numbers(1_000_000, 0, 0, 0x10, 2, 0, 1, 0, 16, 16, 0, 0, 0, 0)) + chunk(b"LIST", strl)
    movi, index = bytearray(b"movi"), bytearray()
    for color in [(0, 0, 255, 0), (255, 0, 0, 0)]:
        frame = bytes(color) * 16 * 16
        index.extend(b"00db" + numbers(0x10, len(movi), len(frame)))
        movi.extend(chunk(b"00db", frame))
    return chunk(b"RIFF", b"AVI " + chunk(b"LIST", header) + chunk(b"LIST", movi) + chunk(b"idx1", index))


def seven_copy(name, contents):
    """One small COPY block with a CRC, matching the Rust archive fixtures."""
    names = b"\0" + (name + "\0").encode("utf-16le")
    assert len(contents) < 128 and len(names) < 128
    header = bytearray([1, 4, 6, 0, 1, 9, len(contents), 0,
                        7, 11, 1, 0, 1, 1, 0, 12, len(contents), 10, 1])
    header.extend(struct.pack("<I", binascii.crc32(contents)))
    header.extend([0, 8, 13, 1, 0, 0, 5, 1, 17, len(names)])
    header.extend(names)
    header.extend([0, 0])
    start = struct.pack("<QQI", len(contents), len(header), binascii.crc32(header))
    return (b"7z\xbc\xaf\x27\x1c\0\x04"
            + struct.pack("<I", binascii.crc32(start)) + start + contents + header)


def iso_image():
    """The small ISO9660 ROOT.TXT/DOCS/README.TXT fixture used by Rust tests."""
    def record(name, sector, length, directory):
        data = bytearray(33 + len(name) + (len(name) % 2 == 0))
        data[0] = len(data)
        struct.pack_into("<I", data, 2, sector)
        struct.pack_into(">I", data, 6, sector)
        struct.pack_into("<I", data, 10, length)
        struct.pack_into(">I", data, 14, length)
        data[18:25] = bytes([126, 10, 10, 12, 0, 0, 0])
        data[25] = 2 if directory else 0
        data[28:32] = b"\1\0\0\1"
        data[32] = len(name)
        data[33:33 + len(name)] = name
        return data

    data = bytearray(32 * 2048)
    primary = bytearray(2048)
    primary[:7] = b"\1CD001\1"
    primary[8:72] = b" " * 64
    primary[40:45] = b"OXYDE"
    primary[80:88] = struct.pack("<I", 32) + struct.pack(">I", 32)
    for offset in [120, 124]:
        primary[offset:offset + 4] = b"\1\0\0\1"
    primary[128:132] = struct.pack("<H", 2048) + struct.pack(">H", 2048)
    primary[132:140] = struct.pack("<I", 22) + struct.pack(">I", 22)
    struct.pack_into("<I", primary, 140, 18)
    struct.pack_into(">I", primary, 148, 19)
    primary[156:190] = record(b"\0", 20, 2048, True)
    primary[190:813] = b" " * 623
    for offset in [813, 830, 847, 864]:
        primary[offset:offset + 16] = b"0" * 16
    primary[881] = 1
    data[16 * 2048:17 * 2048] = primary
    data[17 * 2048:17 * 2048 + 7] = b"\xffCD001\1"
    for sector, endian in [(18, "<"), (19, ">")]:
        for offset, name, extent in [(0, b"\0", 20), (10, b"DOCS", 21)]:
            pos = sector * 2048 + offset
            data[pos] = len(name)
            struct.pack_into(endian + "I", data, pos + 2, extent)
            struct.pack_into(endian + "H", data, pos + 6, 1)
            data[pos + 8:pos + 8 + len(name)] = name
    for sector, records in [
        (20, [record(b"\0", 20, 2048, True), record(b"\1", 20, 2048, True),
              record(b"DOCS", 21, 2048, True), record(b"ROOT.TXT;1", 22, 4, False)]),
        (21, [record(b"\0", 21, 2048, True), record(b"\1", 20, 2048, True),
              record(b"README.TXT;1", 23, 11, False)]),
    ]:
        pos = sector * 2048
        for entry in records:
            data[pos:pos + len(entry)] = entry
            pos += len(entry)
    data[22 * 2048:22 * 2048 + 4] = b"root"
    data[23 * 2048:23 * 2048 + 11] = b"ISO content"
    return data


def main():
    project = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--launch-cache-unavailable", type=Path,
                        help="Launch Oxyde with the generated private TEMP directory")
    args = parser.parse_args()
    if args.launch_cache_unavailable:
        base = args.launch_cache_unavailable.resolve(strict=True)
        if not base.is_relative_to(project / "target"):
            parser.error("Fixture directory must be inside this project's target")
        cache = base / "cache-indisponible"
        if not (cache / "oxyde-archive-open").is_file():
            parser.error("Expected the generated cache-unavailable fixture")
        binary = project / "target" / "debug" / "oxyde.exe"
        if not binary.is_file():
            parser.error("Build Oxyde first: cargo build --bin oxyde --locked --offline")
        environment = os.environ.copy()
        environment.update(TEMP=str(cache), TMP=str(cache))
        process = subprocess.Popen([str(binary)], cwd=project, env=environment,
                                   creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        print(f"Oxyde pid={process.pid}; private TEMP={cache}")
        print("Only this child process uses the private TEMP; close it after the check.")
        return
    target = project / "target"
    target.mkdir(exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix="security-visual-check-", dir=target))
    previews = base / "aperçus"
    archives = base / "archives"
    source = base / "déplacements" / "source"
    destination = base / "déplacements" / "destination"
    cache = base / "cache-indisponible"
    exported = base / "export-permanent"
    trash = base / "corbeille"
    for directory in (previews, archives, source, destination, cache, exported, trash):
        directory.mkdir(parents=True)
    (previews / "01-normal.png").write_bytes(png(64, 64, (25, 170, 90, 180)))
    (previews / "02-limite-dimensions.png").write_bytes(png(32769, 1, (20, 160, 90, 255)))
    (previews / "03-corrompu.png").write_bytes(b"Not an image; inert test fixture.\n")
    (previews / "04-animation.gif").write_bytes(gif(12))
    (previews / "05-animation-limitee.gif").write_bytes(gif(257))
    (previews / "06-proportions.svg").write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="300" '
        'viewBox="0 0 100 300"><rect width="100" height="300" fill="#27ae60"/>'
        '<rect x="20" y="20" width="60" height="260" fill="#2980b9"/></svg>\n',
        encoding="utf-8")
    (previews / "07-page.pdf").write_bytes(pdf())
    with zipfile.ZipFile(previews / "08-office.odt", "w") as document:
        document.writestr("mimetype", "application/vnd.oasis.opendocument.text")
        document.writestr("content.xml", '<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2"><office:body><office:text><text:p>École : aperçu Office.</text:p></office:text></office:body></office:document-content>')
        document.writestr("Thumbnails/thumbnail.png", png(64, 32, (200, 30, 20, 255)))
        document.writestr("META-INF/manifest.xml", '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="Thumbnails/thumbnail.png" manifest:media-type="image/png"/></manifest:manifest>')
    (previews / "09-video.avi").write_bytes(video())
    with zipfile.ZipFile(archives / "01-normal.zip", "w") as archive:
        archive.writestr("lecture.txt", "Archive normale, contenu inoffensif.\n")
        archive.writestr("école_日本語/note.txt", "Nom Unicode.\n")
    with zipfile.ZipFile(archives / "02-limite-profondeur.zip", "w") as archive:
        archive.writestr("/".join(["d"] * 64 + ["note.txt"]), "x")
    # Small navigation/search/open/extract samples. No helper executable,
    # optional Python package, external document or large payload is needed.
    contents = "Archive normale, contenu inoffensif.\n".encode("utf-8")
    for suffix, mode in [("tar", "w"), ("tar.gz", "w:gz")]:
        with tarfile.open(archives / f"03-normal.{suffix}", mode) as archive:
            for name in ["lecture.txt", "école_日本語/note.txt"]:
                entry = tarfile.TarInfo(name)
                entry.size = len(contents)
                entry.mtime = 1_234_567_890
                archive.addfile(entry, io.BytesIO(contents))
    (archives / "04-normal.7z").write_bytes(seven_copy("école_日本語/note.txt", contents))
    (archives / "05-normal.iso").write_bytes(iso_image())
    (archives / "06-corrompu.zip").write_bytes(b"Inert damaged archive fixture.\n")
    (source / "témoin.txt").write_text("Fichier témoin du déplacement.\n", encoding="utf-8")
    (cache / "oxyde-archive-open").write_text("Inert obstruction in private TEMP.\n", encoding="utf-8")
    (exported / "lecture.txt").write_text("TÉMOIN À CONSERVER\n", encoding="utf-8")
    for name in ("Annuler école.txt", "Conflit école.txt", "Rétablir école.txt"):
        (trash / name).write_text("ÉLÉMENT ORIGINAL À RESTAURER\n", encoding="utf-8")
    (trash / "témoin-remplacement.txt").write_text("REMPLACEMENT À CONSERVER\n", encoding="utf-8")
    (trash / "Lien école.url").write_text("[InternetShortcut]\nURL=https://example.invalid/\n", encoding="utf-8")
    (trash / "Dossier école").mkdir()
    (trash / "Dossier école" / "contenu.txt").write_text("CONTENU DU DOSSIER À RESTAURER\n", encoding="utf-8")
    (trash / "page.html").write_text("<!doctype html><p>Témoin inoffensif.</p>\n", encoding="utf-8")
    (trash / "page_files").mkdir()
    (trash / "page_files" / "contenu.txt").write_text("DOSSIER COMPAGNON À CONSERVER\n", encoding="utf-8")
    manifest = {str(path.relative_to(base)): path.stat().st_size
                for path in base.rglob("*") if path.is_file()}
    total = sum(manifest.values())
    assert total < 1024 * 1024, "Fixtures must remain below 1 MiB"
    (base / "manifest.json").write_text(
        json.dumps({"total_bytes": total, "files": manifest}, ensure_ascii=False, indent=2),
        encoding="utf-8")
    print(base)
    print(f"{len(manifest)} files; {total} bytes; no existing file modified.")


if __name__ == "__main__":
    main()
