//! Retrait des métadonnées d'un fichier exporté (idée de l'utilisateur, 25/11/2026 :
//! un export pour le web sans rien d'autre que l'image, quelques octets de moins et ni
//! boîtier, ni date, ni position GPS).
//!
//! **Sans recompression** : seuls les blocs de métadonnées sont retirés, les pixels
//! restent octet pour octet. Gardé : le **profil couleur** (ICC, sans lui les couleurs
//! changeraient) et l'**orientation** (une image tournée par sa balise le resterait).
//! Retiré : EXIF (boîtier, réglages, dates, GPS), XMP (notes, auteur…), IPTC,
//! commentaires et textes.
//!
//! - JPEG : segments `APP1` (EXIF, XMP), `APP13` (IPTC), `COM`, les `APPn` autres que
//!   JFIF, ICC et Adobe, et ce qui suit la fin de l'image (aperçus MPF) ; une
//!   orientation autre que 1 est remise dans un EXIF minimal.
//! - PNG : morceaux `eXIf`, `tEXt`, `zTXt`, `iTXt` (XMP), `tIME`.
//! - TIFF : balises de description de l'IFD0 (boîtier, logiciel, date, auteur, droits,
//!   XMP, IPTC, Photoshop, sous-IFD EXIF et GPS) ; leurs données sont aussi effacées
//!   (mises à zéro) dans le fichier, pas seulement oubliées.
//!
//! Voir `doc/en/export.md`.

use std::path::Path;

/// Retire les métadonnées de `path` (JPEG, PNG ou TIFF, d'après son contenu), sur
/// place. Un autre format : erreur.
pub fn strip(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let out = strip_bytes(&bytes)?;
    if out != bytes {
        std::fs::write(path, out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// [`strip`] sur le contenu d'un fichier.
pub fn strip_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let unreadable = || crate::t!("strip-unreadable");
    match bytes {
        [0xFF, 0xD8, ..] => strip_jpeg(bytes).ok_or_else(unreadable),
        [0x89, b'P', b'N', b'G', ..] => strip_png(bytes).ok_or_else(unreadable),
        [b'I', b'I', 42, 0, ..] | [b'M', b'M', 0, 42, ..] => {
            strip_tiff(bytes).ok_or_else(unreadable)
        }
        _ => Err(crate::t!("strip-unsupported")),
    }
}

/// JPEG : les segments gardés, l'image jusqu'à sa fin, puis l'orientation remise.
fn strip_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    let segments = crate::jpeg::segments(bytes)?;
    let orientation = crate::jpeg::orientation(bytes).unwrap_or(1);
    let mut out = vec![0xFF, 0xD8];
    let mut scan = None;
    for (marker, start, end) in segments {
        let id = |tag: &[u8]| bytes.get(start + 4..start + 4 + tag.len()) == Some(tag);
        let keep = match marker {
            0xE0 => id(b"JFIF\0"),
            0xE2 => id(b"ICC_PROFILE\0"),
            0xE1 | 0xE3..=0xED | 0xEF | 0xFE => false,
            _ => true,
        };
        if keep {
            out.extend_from_slice(&bytes[start..end]);
        }
        if marker == 0xDA {
            scan = Some(end);
        }
    }
    // Données compressées jusqu'à la fin de l'image (EOI) : ce qui suit (aperçus d'un
    // MPF) part aussi.
    let end = end_of_image(bytes, scan?)?;
    out.extend_from_slice(&bytes[scan?..end]);
    if orientation != 1 {
        return crate::jpeg::set_orientation(&out, orientation);
    }
    Some(out)
}

/// Position juste après le marqueur de fin d'image (EOI), en partant des données
/// compressées qui suivent un SOS ; les segments entre deux balayages (JPEG progressif)
/// sont sautés.
fn end_of_image(bytes: &[u8], mut at: usize) -> Option<usize> {
    while at + 1 < bytes.len() {
        if bytes[at] != 0xFF {
            at += 1;
            continue;
        }
        match bytes[at + 1] {
            // Octet échappé, remplissage, redémarrage : toujours des données.
            0x00 | 0xFF | 0xD0..=0xD7 => at += if bytes[at + 1] == 0xFF { 1 } else { 2 },
            0xD9 => return Some(at + 2),
            _ => {
                let len = usize::from(u16::from_be_bytes([
                    *bytes.get(at + 2)?,
                    *bytes.get(at + 3)?,
                ]));
                at += 2 + len;
            }
        }
    }
    // Pas de fin trouvée (fichier tronqué) : tout est gardé.
    Some(bytes.len())
}

/// PNG : les morceaux de texte et de métadonnées retirés, les autres tels quels.
fn strip_png(bytes: &[u8]) -> Option<Vec<u8>> {
    const DROP: [&[u8; 4]; 5] = [b"eXIf", b"tEXt", b"zTXt", b"iTXt", b"tIME"];
    let mut out = bytes.get(..8)?.to_vec();
    let mut at = 8;
    while at < bytes.len() {
        let len = u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize;
        let end = at.checked_add(12)?.checked_add(len)?;
        let kind = bytes.get(at + 4..at + 8)?;
        if end > bytes.len() {
            return None;
        }
        if !DROP.iter().any(|d| kind == &d[..]) {
            out.extend_from_slice(&bytes[at..end]);
        }
        at = end;
        if kind == b"IEND" {
            break;
        }
    }
    Some(out)
}

/// Balises de description de l'IFD0 d'un TIFF, retirées : description, boîtier,
/// modèle, logiciel, date, auteur, ordinateur, droits, XMP, IPTC, Photoshop, sous-IFD
/// EXIF et GPS, titres et auteurs Windows, identifiant unique.
const TIFF_DROP: [u16; 18] = [
    270, 271, 272, 305, 306, 315, 316, 33432, 700, 33723, 34377, 34665, 34853, 40091, 40092, 40093,
    40094, 42016,
];

/// Sous-IFD dont le contenu est effacé avec la balise qui y mène (EXIF, GPS).
const TIFF_SUB_IFDS: [u16; 2] = [34665, 34853];

/// Lecture d'un TIFF selon son ordre des octets.
struct Tiff<'a> {
    bytes: &'a [u8],
    big: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.bytes.get(at..at + 2)?.try_into().ok()?;
        Some(if self.big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.bytes.get(at..at + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    /// Entrées de l'IFD en `at` : (position de l'entrée, balise, type, nombre).
    fn entries(&self, at: usize) -> Option<Vec<(usize, u16, u16, u32)>> {
        let count = usize::from(self.u16(at)?);
        (0..count)
            .map(|i| {
                let e = at + 2 + i * 12;
                Some((e, self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)?))
            })
            .collect()
    }

    /// Données d'une entrée rangées hors de l'entrée (plus de 4 octets) : (début, fin).
    fn outside(&self, entry: usize, kind: u16, count: u32) -> Option<(usize, usize)> {
        let size = match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 => 8,
            _ => return None,
        };
        let len = size * count as usize;
        if len <= 4 {
            return None;
        }
        let start = self.u32(entry + 8)? as usize;
        Some((start, start.checked_add(len)?))
    }
}

/// TIFF : l'IFD0 récrit sur place sans les balises de [`TIFF_DROP`], et leurs données
/// (et celles des sous-IFD EXIF et GPS) mises à zéro. Les données de l'image ne bougent
/// pas.
fn strip_tiff(bytes: &[u8]) -> Option<Vec<u8>> {
    let tiff = Tiff {
        bytes,
        big: bytes.get(..2)? == b"MM",
    };
    let ifd = tiff.u32(4)? as usize;
    let entries = tiff.entries(ifd)?;
    let next = tiff.u32(ifd + 2 + entries.len() * 12)?;
    let mut out = bytes.to_vec();
    let mut zero = |range: (usize, usize)| -> Option<()> {
        out.get_mut(range.0..range.1)?.fill(0);
        Some(())
    };
    let mut kept = Vec::new();
    for &(e, tag, kind, count) in &entries {
        if !TIFF_DROP.contains(&tag) {
            kept.push(bytes.get(e..e + 12)?.to_vec());
            continue;
        }
        if let Some(range) = tiff.outside(e, kind, count) {
            zero(range)?;
        }
        if TIFF_SUB_IFDS.contains(&tag) {
            let sub = tiff.u32(e + 8)? as usize;
            if let Some(sub_entries) = tiff.entries(sub) {
                for &(se, _, skind, scount) in &sub_entries {
                    if let Some(range) = tiff.outside(se, skind, scount) {
                        zero(range)?;
                    }
                }
                zero((sub, sub + 2 + sub_entries.len() * 12 + 4))?;
            }
        }
    }
    // IFD0 récrit à la même place (plus court) ; le reste de l'ancien, à zéro.
    let old_end = ifd + 2 + entries.len() * 12 + 4;
    zero((ifd, old_end))?;
    let count = u16::try_from(kept.len()).ok()?;
    let mut ifd_bytes = if tiff.big {
        count.to_be_bytes().to_vec()
    } else {
        count.to_le_bytes().to_vec()
    };
    for entry in kept {
        ifd_bytes.extend_from_slice(&entry);
    }
    ifd_bytes.extend_from_slice(&if tiff.big {
        next.to_be_bytes()
    } else {
        next.to_le_bytes()
    });
    out.get_mut(ifd..ifd + ifd_bytes.len())?
        .copy_from_slice(&ifd_bytes);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un JPEG 8 × 8 encodé par `image`.
    fn jpeg() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 30, 30]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
        out.into_inner()
    }

    /// `bytes` avec le segment `segment` inséré juste après SOI.
    fn with_segment(bytes: &[u8], marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = bytes[..2].to_vec();
        out.extend_from_slice(&[0xFF, marker]);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&bytes[2..]);
        out
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn a_jpeg_keeps_its_pixels_colours_and_orientation_and_loses_the_rest() {
        let plain = jpeg();
        let tagged = crate::jpeg::set_orientation(&plain, 6).unwrap();
        let tagged = crate::jpeg::with_icc(&tagged, b"fake icc profile").unwrap();
        let tagged = with_segment(
            &tagged,
            0xE1,
            b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>Ana Lima</x:xmpmeta>",
        );
        let tagged = with_segment(&tagged, 0xED, b"Photoshop 3.0\0IPTC secret");
        let tagged = with_segment(&tagged, 0xFE, b"a comment");
        let mut tagged = tagged;
        tagged.extend_from_slice(b"\xFF\xD8trailing preview");
        let out = strip_bytes(&tagged).unwrap();
        assert!(!contains(&out, b"Ana Lima") && !contains(&out, b"IPTC secret"));
        assert!(!contains(&out, b"a comment") && !contains(&out, b"trailing preview"));
        assert_eq!(
            crate::jpeg::icc_profile(&out).as_deref(),
            Some(&b"fake icc profile"[..])
        );
        assert_eq!(
            crate::jpeg::orientation(&out),
            Some(6),
            "orientation gardée"
        );
        let a = image::load_from_memory(&tagged).unwrap().to_rgb8();
        let b = image::load_from_memory(&out).unwrap().to_rgb8();
        assert_eq!(a, b, "pixels identiques");
        // Sans orientation à garder : plus d'EXIF du tout.
        let out = strip_bytes(&with_segment(&plain, 0xE1, b"Exif\0\0junk")).unwrap();
        assert!(!contains(&out, b"Exif"));
    }

    #[test]
    fn a_png_loses_its_text_chunks() {
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        // Un morceau tEXt juste après IHDR (8 + 25 octets).
        let text = b"tEXtAuthor\0Ana Lima";
        let mut chunk = ((text.len() - 4) as u32).to_be_bytes().to_vec();
        chunk.extend_from_slice(text);
        chunk.extend_from_slice(&[0, 0, 0, 0]); // CRC, non vérifié ici
        let mut tagged = png[..33].to_vec();
        tagged.extend_from_slice(&chunk);
        tagged.extend_from_slice(&png[33..]);
        let out = strip_bytes(&tagged).unwrap();
        assert!(!contains(&out, b"Ana Lima"));
        assert_eq!(out, png, "le reste octet pour octet");
    }

    #[test]
    fn a_tiff_loses_its_description_tags_and_their_data() {
        // TIFF petit-boutiste : 1 × 1 pixel gris, IFD0 avec structure + Make + Artist.
        let mut t: Vec<u8> = b"II\x2A\0".to_vec();
        t.extend_from_slice(&8u32.to_le_bytes());
        let entries: Vec<(u16, u16, u32, u32)> = vec![
            (256, 3, 1, 1),   // largeur
            (257, 3, 1, 1),   // hauteur
            (258, 3, 1, 8),   // bits
            (262, 3, 1, 1),   // noir = 0
            (271, 2, 9, 200), // Make → 200
            (273, 4, 1, 230), // données → 230
            (277, 3, 1, 1),   // composantes
            (279, 4, 1, 1),   // octets de l'image
            (315, 2, 9, 210), // Artist → 210
        ];
        t.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (tag, kind, count, value) in &entries {
            t.extend_from_slice(&tag.to_le_bytes());
            t.extend_from_slice(&kind.to_le_bytes());
            t.extend_from_slice(&count.to_le_bytes());
            t.extend_from_slice(&value.to_le_bytes());
        }
        t.extend_from_slice(&0u32.to_le_bytes());
        t.resize(200, 0);
        t.extend_from_slice(b"FUJIFILM\0\0");
        t.extend_from_slice(b"Ana Lima\0");
        t.resize(230, 0);
        t.push(128);
        let out = strip_bytes(&t).unwrap();
        assert!(!contains(&out, b"FUJIFILM") && !contains(&out, b"Ana Lima"));
        assert_eq!(out.len(), t.len());
        let img = image::load_from_memory(&out).unwrap().to_luma8();
        assert_eq!(img.get_pixel(0, 0).0, [128], "l'image intacte");
    }

    #[test]
    fn other_formats_are_refused() {
        assert!(strip_bytes(b"RIFF....WEBP").is_err());
    }
}
