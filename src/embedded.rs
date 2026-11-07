//! Notes inscrites dans les images elles-mêmes (issue n° 276, décidé pour la bêta le
//! 07/11/2026) : un paquet XMP embarqué, ce que lisent l'Explorateur Windows, Spotlight,
//! Lightroom, darktable, digiKam… Les sidecars restent la référence de `tri` ; ce module
//! lit les notes d'une image sans sidecar (étoiles posées ailleurs) et les y inscrit sur
//! demande.
//!
//! - **Formats** : JPEG (segment APP1 `http://ns.adobe.com/xap/1.0/`), PNG (chunk `iTXt`
//!   `XML:com.adobe.xmp`), WebP (chunk `XMP `), TIFF (balise 700), HEIF / HEIC (élément
//!   `mime`). Jamais un RAW.
//! - **Sans recompression** : seul le bloc de métadonnées change, le reste du fichier est
//!   gardé octet pour octet. Le paquet existant est complété par [`crate::xmp::apply`]
//!   (nos champs seulement, comme pour les sidecars), jamais remplacé.
//! - **EXIF** : une note EXIF `Rating` (0x4746) ou `RatingPercent` (0x4749) déjà présente
//!   est mise à jour sur place (deux octets, sans restructurer l'EXIF), sans quoi
//!   l'Explorateur Windows pourrait montrer l'ancienne ; absente, elle n'est pas ajoutée
//!   (le XMP suffit). À la lecture, elle sert de repli quand le XMP ne dit rien.
//! - **Lecture en flux** : seuls les en-têtes sont lus (segments jusqu'aux données du JPEG,
//!   chunks sautés du PNG et du WebP, IFD0 du TIFF), pour lire tout un dossier vite.
//! - **Écriture** : le nouveau contenu est d'abord copié à côté (`.<nom>.tri.tmp`), puis
//!   écrit à la place de l'image (même fichier : attributs étendus, droits et étiquettes du
//!   Finder gardés), puis relu et vérifié.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::t;
use crate::xmp::{self, Notes};

/// Conteneur d'image qui peut porter des notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// WebP.
    Webp,
    /// TIFF (pas un RAW en TIFF : DNG, NEF… sont exclus par leur extension).
    Tiff,
    /// HEIF / HEIC (iPhone, appareils récents).
    Heif,
}

impl Container {
    /// Le conteneur de `path`, d'après son extension (sans tenir compte de la casse).
    pub fn of(path: &Path) -> Option<Container> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "jpe" => Some(Container::Jpeg),
            "png" => Some(Container::Png),
            "webp" => Some(Container::Webp),
            "tif" | "tiff" => Some(Container::Tiff),
            "heic" | "heif" | "hif" => Some(Container::Heif),
            _ => None,
        }
    }
}

/// Vrai si `path` peut porter des notes embarquées.
pub fn supports(path: &Path) -> bool {
    Container::of(path).is_some()
}

/// Ce que l'en-tête d'une image dit des notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Found {
    /// Paquet XMP embarqué.
    xmp: Option<Vec<u8>>,
    /// Note EXIF `Rating` (0 à 5).
    rating: Option<u16>,
}

/// Notes inscrites dans `path` ; `None` si l'image n'en porte aucune (ni paquet XMP avec
/// des notes, ni note EXIF), ou n'est pas d'un format qui en porte.
pub fn read(path: &Path) -> io::Result<Option<Notes>> {
    let Some(container) = Container::of(path) else {
        return Ok(None);
    };
    let mut file = io::BufReader::new(File::open(path)?);
    let found = match container {
        Container::Jpeg => scan_jpeg(&mut file)?,
        Container::Png => scan_png(&mut file)?,
        Container::Webp => scan_webp(&mut file)?,
        Container::Tiff => scan_tiff(&mut file)?,
        Container::Heif => scan_heif(&mut file)?,
    };
    Ok(notes_of(&found))
}

/// Notes de ce qui a été trouvé : le XMP d'abord, la note EXIF à défaut.
fn notes_of(found: &Found) -> Option<Notes> {
    let from_xmp = found
        .xmp
        .as_deref()
        .and_then(|x| std::str::from_utf8(x).ok())
        .and_then(|x| xmp::parse(x).ok())
        .filter(|n| *n != Notes::default());
    if from_xmp.is_some() {
        return from_xmp;
    }
    let stars = found.rating.filter(|&r| r > 0)?.min(5) as u8;
    let mut notes = Notes::default();
    notes.set_stars(stars);
    Some(notes)
}

/// Inscrit `notes` dans l'image `path`, relue juste avant. Renvoie vrai si le fichier a
/// changé. Erreur (message traduit) si le format n'en porte pas, si le fichier est mal
/// formé ou si la relecture ne retrouve pas les notes ; l'image n'est alors pas modifiée
/// (ou, si l'écriture elle-même a échoué, sa copie `.tri.tmp` est gardée à côté).
pub fn write(path: &Path, notes: &Notes) -> Result<bool, String> {
    let io = |e: io::Error| t!("embed-io", error = e.to_string());
    let Some(updated) = updated(path, notes)? else {
        return Ok(false);
    };
    let tmp = tmp_path(path);
    std::fs::write(&tmp, &updated).map_err(io)?;
    if let Err(e) = std::fs::write(path, &updated) {
        return Err(t!(
            "embed-write-failed",
            error = e.to_string(),
            copy = tmp.display().to_string()
        ));
    }
    let _ = std::fs::remove_file(&tmp);
    // Relu : ce qui compte est ce qui est sur le disque.
    let back = read(path).map_err(io)?.unwrap_or_default();
    if !same_notes(&back, notes) {
        return Err(t!("embed-verify-failed"));
    }
    Ok(true)
}

/// Vrai si [`write`] changerait l'image (`--dry-run`) ; mêmes erreurs.
pub fn would_change(path: &Path, notes: &Notes) -> Result<bool, String> {
    updated(path, notes).map(|u| u.is_some())
}

/// Contenu de `path` avec `notes` inscrites ; `None` s'il ne change pas.
fn updated(path: &Path, notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    let container = Container::of(path).ok_or_else(|| t!("embed-unsupported"))?;
    let bytes = std::fs::read(path).map_err(|e| t!("embed-io", error = e.to_string()))?;
    match container {
        Container::Jpeg => update_jpeg(&bytes, notes),
        Container::Png => update_png(&bytes, notes),
        Container::Webp => update_webp(&bytes, notes),
        Container::Tiff => update_tiff(&bytes, notes),
        Container::Heif => update_heif(&bytes, notes),
    }
}

/// Notes équivalentes une fois relues (étoiles, drapeau, couleurs).
fn same_notes(a: &Notes, b: &Notes) -> bool {
    a.stars == b.stars && a.flag == b.flag && a.colors == b.colors
}

/// Copie de sécurité de `path` pendant l'écriture : `.<nom>.tri.tmp`, à côté.
fn tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.tri.tmp"))
}

/// Paquet XMP mis à jour avec `notes` (`existing` complété, ou un paquet neuf) ; `None`
/// s'il ne change pas.
fn updated_packet(existing: Option<&[u8]>, notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    // Pas de paquet créé pour ne rien dire : une image jamais notée reste telle quelle.
    if existing.is_none() && *notes == Notes::default() {
        return Ok(None);
    }
    let base = match existing {
        Some(bytes) => std::str::from_utf8(bytes)
            .map_err(|_| t!("embed-xmp-not-utf8"))?
            .to_string(),
        None => new_packet(),
    };
    let updated = xmp::apply(&base, notes).map_err(|e| e.to_string())?;
    if existing.is_some() && updated == base {
        return Ok(None);
    }
    Ok(Some(updated.into_bytes()))
}

/// Paquet XMP neuf, dans son enveloppe `xpacket` (forme des paquets embarqués).
fn new_packet() -> String {
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n{}<?xpacket end=\"w\"?>",
        xmp::new_packet_body()
    )
}

/// Note EXIF qui correspond à `notes` : `Rating` 0 à 5 (0 pour une photo rejetée, que
/// l'EXIF ne connaît pas), et `RatingPercent` comme Windows (1, 25, 50, 75, 99).
fn exif_rating(notes: &Notes) -> (u16, u16) {
    let stars = if notes.flag == xmp::Flag::Rejected {
        0
    } else {
        u16::from(notes.stars.min(5))
    };
    let percent = [0, 1, 25, 50, 75, 99][usize::from(stars)];
    (stars, percent)
}

// --- TIFF (aussi la structure de l'EXIF) -------------------------------------------------

/// Balise `XMLPacket` (XMP d'un TIFF).
const TAG_XMP: u16 = 700;
/// Balise EXIF `Rating`.
const TAG_RATING: u16 = 0x4746;
/// Balise EXIF `RatingPercent`.
const TAG_RATING_PERCENT: u16 = 0x4749;

/// Une entrée de l'IFD0.
#[derive(Clone, Copy, Debug)]
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// Position (dans le TIFF) des 4 octets de la valeur ou de son adresse.
    value_at: usize,
}

/// Lecture d'un TIFF en mémoire (l'EXIF d'un JPEG, d'un PNG, d'un WebP).
struct Tiff<'a> {
    bytes: &'a [u8],
    big: bool,
}

impl<'a> Tiff<'a> {
    fn new(bytes: &'a [u8]) -> Option<Tiff<'a>> {
        let big = match bytes.get(..4)? {
            b"MM\0*" => true,
            b"II*\0" => false,
            _ => return None,
        };
        Some(Tiff { bytes, big })
    }
    fn u16_at(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.bytes.get(at..at + 2)?.try_into().ok()?;
        Some(if self.big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }
    fn u32_at(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.bytes.get(at..at + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }
    /// Adresse de l'IFD0, ses entrées et l'adresse de l'IFD suivant.
    fn ifd0(&self) -> Option<(usize, Vec<Entry>, u32)> {
        let at = self.u32_at(4)? as usize;
        let n = usize::from(self.u16_at(at)?);
        let entries = (0..n)
            .map(|i| {
                let e = at + 2 + 12 * i;
                Some(Entry {
                    tag: self.u16_at(e)?,
                    kind: self.u16_at(e + 2)?,
                    count: self.u32_at(e + 4)?,
                    value_at: e + 8,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let next = self.u32_at(at + 2 + 12 * n)?;
        Some((at, entries, next))
    }
    /// Valeur entière d'une entrée SHORT ou LONG d'une seule valeur.
    fn number(&self, e: &Entry) -> Option<u32> {
        match (e.kind, e.count) {
            (3, 1) => self.u16_at(e.value_at).map(u32::from),
            (4, 1) => self.u32_at(e.value_at),
            _ => None,
        }
    }
    /// Octets de la valeur d'une entrée BYTE ou UNDEFINED (le XMP d'un TIFF).
    fn data(&self, e: &Entry) -> Option<&'a [u8]> {
        let len = e.count as usize;
        let at = if len <= 4 {
            e.value_at
        } else {
            self.u32_at(e.value_at)? as usize
        };
        self.bytes.get(at..at.checked_add(len)?)
    }
}

/// Note EXIF `Rating` de l'IFD0 d'un TIFF en mémoire.
fn tiff_rating(tiff: &[u8]) -> Option<u16> {
    let t = Tiff::new(tiff)?;
    let (_, entries, _) = t.ifd0()?;
    let e = entries.iter().find(|e| e.tag == TAG_RATING)?;
    t.number(e).and_then(|n| u16::try_from(n).ok())
}

/// Met à jour sur place, dans l'IFD0 de `tiff`, les notes EXIF déjà présentes (SHORT ou
/// LONG d'une valeur). Vrai si un octet a changé.
fn patch_tiff_rating(tiff: &mut [u8], notes: &Notes) -> bool {
    let Some(t) = Tiff::new(tiff) else {
        return false;
    };
    let big = t.big;
    let Some((_, entries, _)) = t.ifd0() else {
        return false;
    };
    let (stars, percent) = exif_rating(notes);
    let mut changed = false;
    for e in entries {
        let value = match e.tag {
            TAG_RATING => stars,
            TAG_RATING_PERCENT => percent,
            _ => continue,
        };
        let bytes: Vec<u8> = match (e.kind, e.count, big) {
            (3, 1, true) => value.to_be_bytes().to_vec(),
            (3, 1, false) => value.to_le_bytes().to_vec(),
            (4, 1, true) => u32::from(value).to_be_bytes().to_vec(),
            (4, 1, false) => u32::from(value).to_le_bytes().to_vec(),
            _ => continue,
        };
        let slot = &mut tiff[e.value_at..e.value_at + bytes.len()];
        if slot != bytes.as_slice() {
            slot.copy_from_slice(&bytes);
            changed = true;
        }
    }
    changed
}

/// En-tête d'un fichier TIFF : le XMP (balise 700) et la note EXIF de l'IFD0, lus par
/// sauts (l'image elle-même n'est pas lue).
fn scan_tiff<R: Read + Seek>(r: &mut R) -> io::Result<Found> {
    let mut head = [0u8; 8];
    r.read_exact(&mut head)?;
    let big = match &head[..4] {
        b"MM\0*" => true,
        b"II*\0" => false,
        _ => return Ok(Found::default()),
    };
    let u16_of = |b: [u8; 2]| {
        if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        }
    };
    let u32_of = |b: [u8; 4]| {
        if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };
    let ifd = u32_of(head[4..8].try_into().unwrap_or_default());
    r.seek(SeekFrom::Start(u64::from(ifd)))?;
    let mut n = [0u8; 2];
    r.read_exact(&mut n)?;
    let n = usize::from(u16_of(n));
    let mut table = vec![0u8; 12 * n];
    r.read_exact(&mut table)?;
    let mut found = Found::default();
    for e in table.as_chunks::<12>().0 {
        let tag = u16_of([e[0], e[1]]);
        let kind = u16_of([e[2], e[3]]);
        let count = u32_of([e[4], e[5], e[6], e[7]]);
        let value = [e[8], e[9], e[10], e[11]];
        match tag {
            TAG_RATING if count == 1 => {
                found.rating = match kind {
                    3 => Some(u16_of([value[0], value[1]])),
                    4 => u16::try_from(u32_of(value)).ok(),
                    _ => None,
                };
            }
            TAG_XMP if matches!(kind, 1 | 7) => {
                let len = count as usize;
                if len <= 4 {
                    found.xmp = Some(value[..len].to_vec());
                } else if len <= MAX_XMP {
                    r.seek(SeekFrom::Start(u64::from(u32_of(value))))?;
                    let mut xmp = vec![0u8; len];
                    r.read_exact(&mut xmp)?;
                    found.xmp = Some(xmp);
                }
            }
            _ => {}
        }
    }
    Ok(found)
}

/// Taille au-delà de laquelle un paquet XMP n'est pas lu (garde-fou contre un fichier
/// abîmé).
const MAX_XMP: usize = 16 << 20;

/// TIFF mis à jour : le paquet XMP réécrit à sa place s'il y tient (complété d'espaces
/// dans le paquet), sinon ajouté en fin de fichier avec une copie de l'IFD0 qui le désigne
/// (l'ancien IFD0 reste, inutilisé : les données qu'il désigne ne bougent pas) ; les notes
/// EXIF présentes, sur place.
fn update_tiff(bytes: &[u8], notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    let malformed = || t!("embed-malformed");
    if bytes.get(2..4) == Some(&[0, 43]) || bytes.get(2..4) == Some(&[43, 0]) {
        return Err(t!("embed-bigtiff"));
    }
    let t = Tiff::new(bytes).ok_or_else(malformed)?;
    let big = t.big;
    let (_, entries, next) = t.ifd0().ok_or_else(malformed)?;
    let old = entries.iter().find(|e| e.tag == TAG_XMP);
    let existing = old.and_then(|e| t.data(e));
    let packet = updated_packet(existing, notes)?;
    let mut out = bytes.to_vec();
    let mut changed = patch_tiff_rating(&mut out, notes);
    let Some(packet) = packet else {
        return Ok(changed.then_some(out));
    };
    changed = true;
    let u16b = |v: u16| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let u32b = |v: u32| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    // À sa place, s'il y tient.
    if let Some(e) = old
        && e.count as usize > 4
        && packet.len() <= e.count as usize
        && let Some(padded) = pad_packet(&packet, e.count as usize)
    {
        let at = t.u32_at(e.value_at).ok_or_else(malformed)? as usize;
        out[at..at + padded.len()].copy_from_slice(&padded);
        return Ok(changed.then_some(out));
    }
    // Sinon : le paquet, puis un nouvel IFD0, en fin de fichier (adresses paires).
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let packet_at = u32::try_from(out.len()).map_err(|_| t!("embed-too-large"))?;
    out.extend_from_slice(&packet);
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let ifd_at = u32::try_from(out.len()).map_err(|_| t!("embed-too-large"))?;
    let mut table: Vec<[u8; 12]> = entries
        .iter()
        .filter(|e| e.tag != TAG_XMP)
        .map(|e| {
            let start = e.value_at - 8;
            let mut raw = [0u8; 12];
            raw.copy_from_slice(&out[start..start + 12]);
            (e.tag, raw)
        })
        .map(|(_, raw)| raw)
        .collect();
    let len = u32::try_from(packet.len()).map_err(|_| t!("embed-too-large"))?;
    let mut xmp_entry = [0u8; 12];
    xmp_entry[..2].copy_from_slice(&u16b(TAG_XMP));
    xmp_entry[2..4].copy_from_slice(&u16b(7));
    xmp_entry[4..8].copy_from_slice(&u32b(len));
    xmp_entry[8..].copy_from_slice(&u32b(packet_at));
    table.push(xmp_entry);
    let tag_of = |raw: &[u8; 12]| {
        if big {
            u16::from_be_bytes([raw[0], raw[1]])
        } else {
            u16::from_le_bytes([raw[0], raw[1]])
        }
    };
    table.sort_by_key(tag_of);
    let n = u16::try_from(table.len()).map_err(|_| malformed())?;
    out.extend_from_slice(&u16b(n));
    for raw in &table {
        out.extend_from_slice(raw);
    }
    out.extend_from_slice(&u32b(next));
    out[4..8].copy_from_slice(&u32b(ifd_at));
    Ok(Some(out))
}

/// `packet` complété à `len` octets par des espaces avant `<?xpacket end`, comme le
/// remplissage prévu par la norme ; `None` sans fin de paquet où les mettre.
fn pad_packet(packet: &[u8], len: usize) -> Option<Vec<u8>> {
    if packet.len() == len {
        return Some(packet.to_vec());
    }
    let end = find(packet, b"<?xpacket end")?;
    let mut out = packet[..end].to_vec();
    out.resize(end + len - packet.len(), b' ');
    out.extend_from_slice(&packet[end..]);
    Some(out)
}

/// Position de `needle` dans `haystack` (dernière occurrence).
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

// --- JPEG ---------------------------------------------------------------------------------

/// En-tête du segment APP1 qui porte le XMP.
const JPEG_XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// En-tête du segment APP1 qui porte l'EXIF.
const JPEG_EXIF: &[u8] = b"Exif\0\0";

/// Segments d'un JPEG en mémoire jusqu'aux données de l'image : (marqueur, début, fin).
fn jpeg_segments(bytes: &[u8]) -> Option<Vec<(u8, usize, usize)>> {
    if bytes.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 2;
    loop {
        while bytes.get(at) == Some(&0xFF) && bytes.get(at + 1) == Some(&0xFF) {
            at += 1;
        }
        if bytes.get(at) != Some(&0xFF) {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        let len = usize::from(u16::from_be_bytes([
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]));
        let end = at + 2 + len;
        if len < 2 || end > bytes.len() {
            return None;
        }
        out.push((marker, at, end));
        if marker == 0xDA {
            return Some(out);
        }
        at = end;
    }
}

/// En-tête d'un fichier JPEG : segments lus un à un jusqu'aux données de l'image.
fn scan_jpeg<R: Read + Seek>(r: &mut R) -> io::Result<Found> {
    let mut found = Found::default();
    let mut soi = [0u8; 2];
    r.read_exact(&mut soi)?;
    if soi != [0xFF, 0xD8] {
        return Ok(found);
    }
    loop {
        let mut marker = [0u8; 2];
        r.read_exact(&mut marker)?;
        while marker[1] == 0xFF {
            r.read_exact(&mut marker[1..])?;
        }
        if marker[0] != 0xFF || marker[1] == 0xDA || marker[1] == 0xD9 {
            return Ok(found);
        }
        let mut len = [0u8; 2];
        r.read_exact(&mut len)?;
        let len = usize::from(u16::from_be_bytes(len)).saturating_sub(2);
        if marker[1] == 0xE1 {
            let mut data = vec![0u8; len];
            r.read_exact(&mut data)?;
            if let Some(xmp) = data.strip_prefix(JPEG_XMP) {
                found.xmp.get_or_insert_with(|| xmp.to_vec());
            } else if let Some(exif) = data.strip_prefix(JPEG_EXIF) {
                found.rating = found.rating.or(tiff_rating(exif));
            }
        } else {
            r.seek(SeekFrom::Current(len as i64))?;
        }
    }
}

/// JPEG mis à jour : le segment XMP remplacé à sa place, ou ajouté après les en-têtes JFIF
/// et EXIF ; les notes EXIF présentes, sur place.
fn update_jpeg(bytes: &[u8], notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    let segs = jpeg_segments(bytes).ok_or_else(|| t!("embed-malformed"))?;
    let data = |&(_, s, e): &(u8, usize, usize)| &bytes[s + 4..e];
    let xmp_seg = segs
        .iter()
        .find(|s| s.0 == 0xE1 && data(s).starts_with(JPEG_XMP))
        .copied();
    let existing = xmp_seg.map(|s| &data(&s)[JPEG_XMP.len()..]);
    let packet = updated_packet(existing, notes)?;
    let mut out = bytes.to_vec();
    let mut changed = false;
    if let Some(&(_, s, e)) = segs
        .iter()
        .find(|s| s.0 == 0xE1 && data(s).starts_with(JPEG_EXIF))
    {
        changed = patch_tiff_rating(&mut out[s + 4 + JPEG_EXIF.len()..e], notes);
    }
    let Some(packet) = packet else {
        return Ok(changed.then_some(out));
    };
    let len = 2 + JPEG_XMP.len() + packet.len();
    let len = u16::try_from(len).map_err(|_| t!("embed-too-large"))?;
    let mut segment = vec![0xFF, 0xE1];
    segment.extend_from_slice(&len.to_be_bytes());
    segment.extend_from_slice(JPEG_XMP);
    segment.extend_from_slice(&packet);
    let (start, end) = match xmp_seg {
        Some((_, s, e)) => (s, e),
        None => {
            // Après APP0 (JFIF) et l'EXIF de tête, comme les autres logiciels.
            let at = segs
                .iter()
                .take_while(|s| s.0 == 0xE0 || (s.0 == 0xE1 && data(s).starts_with(JPEG_EXIF)))
                .last()
                .map_or(2, |s| s.2);
            (at, at)
        }
    };
    let mut result = out[..start].to_vec();
    result.extend_from_slice(&segment);
    result.extend_from_slice(&out[end..]);
    Ok(Some(result))
}

// --- PNG ----------------------------------------------------------------------------------

/// Mot-clé du chunk `iTXt` qui porte le XMP.
const PNG_XMP: &[u8] = b"XML:com.adobe.xmp";
/// Signature d'un PNG.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Texte XMP d'un chunk `iTXt` (`None` si c'est un autre texte, ou s'il est compressé).
fn png_xmp(itxt: &[u8]) -> Option<&[u8]> {
    let rest = itxt.strip_prefix(PNG_XMP)?.strip_prefix(b"\0")?;
    // Indicateur de compression, méthode, langue et mot-clé traduit (terminés par 0).
    if *rest.first()? != 0 {
        return None;
    }
    let rest = rest.get(2..)?;
    let lang = rest.iter().position(|&b| b == 0)?;
    let rest = &rest[lang + 1..];
    let translated = rest.iter().position(|&b| b == 0)?;
    Some(&rest[translated + 1..])
}

/// Vrai si le chunk `iTXt` porte un XMP compressé (qu'on ne sait pas compléter).
fn png_xmp_compressed(itxt: &[u8]) -> bool {
    itxt.strip_prefix(PNG_XMP)
        .and_then(|r| r.strip_prefix(b"\0"))
        .and_then(|r| r.first())
        .is_some_and(|&flag| flag != 0)
}

/// En-tête d'un fichier PNG : chunks parcourus par sauts, seuls `iTXt` et `eXIf` lus.
fn scan_png<R: Read + Seek>(r: &mut R) -> io::Result<Found> {
    let mut found = Found::default();
    let mut signature = [0u8; 8];
    r.read_exact(&mut signature)?;
    if signature != PNG_SIGNATURE {
        return Ok(found);
    }
    loop {
        let mut head = [0u8; 8];
        if r.read_exact(&mut head).is_err() {
            return Ok(found);
        }
        let len = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let kind = &head[4..8];
        match kind {
            b"IEND" => return Ok(found),
            b"iTXt" | b"eXIf" if len <= MAX_XMP => {
                let mut data = vec![0u8; len];
                r.read_exact(&mut data)?;
                if kind == b"eXIf" {
                    found.rating = found.rating.or(tiff_rating(&data));
                } else if let Some(xmp) = png_xmp(&data) {
                    found.xmp.get_or_insert_with(|| xmp.to_vec());
                }
                r.seek(SeekFrom::Current(4))?;
            }
            _ => {
                r.seek(SeekFrom::Current(len as i64 + 4))?;
            }
        }
    }
}

/// PNG mis à jour : le chunk XMP remplacé à sa place, ou ajouté après `IHDR` ; la note
/// EXIF présente (`eXIf`), sur place.
fn update_png(bytes: &[u8], notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    use img_parts::Bytes;
    use img_parts::png::{Png, PngChunk};
    let mut png = Png::from_bytes(Bytes::copy_from_slice(bytes))
        .map_err(|e| t!("embed-malformed-detail", error = e.to_string()))?;
    let itxt = |c: &PngChunk| c.kind() == *b"iTXt" && c.contents().starts_with(PNG_XMP);
    if png
        .chunks()
        .iter()
        .any(|c| itxt(c) && png_xmp_compressed(c.contents()))
    {
        return Err(t!("embed-compressed"));
    }
    let position = png.chunks().iter().position(itxt);
    let existing = position.and_then(|i| png_xmp(png.chunks()[i].contents()));
    let packet = updated_packet(existing, notes)?;
    let mut changed = false;
    for chunk in png.chunks_mut() {
        if chunk.kind() == *b"eXIf" {
            let mut exif = chunk.contents().to_vec();
            if patch_tiff_rating(&mut exif, notes) {
                *chunk = PngChunk::new(*b"eXIf", Bytes::from(exif));
                changed = true;
            }
        }
    }
    if let Some(packet) = packet {
        let mut contents = PNG_XMP.to_vec();
        // Fin du mot-clé, non compressé, méthode 0, sans langue ni mot-clé traduit.
        contents.extend_from_slice(&[0, 0, 0, 0, 0]);
        contents.extend_from_slice(&packet);
        let chunk = PngChunk::new(*b"iTXt", Bytes::from(contents));
        match position {
            Some(i) => png.chunks_mut()[i] = chunk,
            None => {
                let after_header = png
                    .chunks()
                    .iter()
                    .position(|c| c.kind() == *b"IHDR")
                    .map_or(0, |i| i + 1);
                png.chunks_mut().insert(after_header, chunk);
            }
        }
        changed = true;
    }
    Ok(changed.then(|| png.encoder().bytes().to_vec()))
}

// --- WebP ---------------------------------------------------------------------------------

/// En-tête d'un fichier WebP : chunks RIFF parcourus par sauts, seuls `XMP ` et `EXIF`
/// lus.
fn scan_webp<R: Read + Seek>(r: &mut R) -> io::Result<Found> {
    let mut found = Found::default();
    let mut head = [0u8; 12];
    r.read_exact(&mut head)?;
    if &head[..4] != b"RIFF" || &head[8..12] != b"WEBP" {
        return Ok(found);
    }
    loop {
        let mut chunk = [0u8; 8];
        if r.read_exact(&mut chunk).is_err() {
            return Ok(found);
        }
        let len = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as usize;
        let padded = len + len % 2;
        match &chunk[..4] {
            b"XMP " | b"EXIF" if len <= MAX_XMP => {
                let mut data = vec![0u8; len];
                r.read_exact(&mut data)?;
                if &chunk[..4] == b"XMP " {
                    found.xmp.get_or_insert(data);
                } else {
                    // EXIF avec ou sans l'en-tête `Exif\0\0`.
                    let tiff = data.strip_prefix(JPEG_EXIF).unwrap_or(&data);
                    found.rating = found.rating.or(tiff_rating(tiff));
                }
                r.seek(SeekFrom::Current((padded - len) as i64))?;
            }
            _ => {
                r.seek(SeekFrom::Current(padded as i64))?;
            }
        }
    }
}

/// WebP mis à jour : le chunk XMP remplacé, ou ajouté en fin (un WebP simple devient
/// étendu, `VP8X`, ce que fait `img-parts`) ; la note EXIF présente, sur place.
fn update_webp(bytes: &[u8], notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    use img_parts::webp::{CHUNK_EXIF, CHUNK_XMP, WebP};
    use img_parts::{Bytes, ImageICC, riff::RiffChunk, riff::RiffContent};
    let mut webp = WebP::from_bytes(Bytes::copy_from_slice(bytes))
        .map_err(|e| t!("embed-malformed-detail", error = e.to_string()))?;
    let existing = webp
        .chunk_by_id(CHUNK_XMP)
        .and_then(|c| c.content().data())
        .map(|d| d.to_vec());
    let packet = updated_packet(existing.as_deref(), notes)?;
    let mut changed = false;
    for chunk in webp.chunks_mut() {
        if chunk.id() == CHUNK_EXIF
            && let RiffContent::Data(data) = chunk.content_mut()
        {
            let mut exif = data.to_vec();
            let offset = if exif.starts_with(JPEG_EXIF) {
                JPEG_EXIF.len()
            } else {
                0
            };
            if patch_tiff_rating(&mut exif[offset..], notes) {
                *data = Bytes::from(exif);
                changed = true;
            }
        }
    }
    if let Some(packet) = packet {
        let chunk = RiffChunk::new(CHUNK_XMP, RiffContent::Data(Bytes::from(packet)));
        match webp.chunks().iter().position(|c| c.id() == CHUNK_XMP) {
            Some(i) => webp.chunks_mut()[i] = chunk,
            None => webp.chunks_mut().push(chunk),
        }
        // Les drapeaux de `VP8X` (et la conversion d'un WebP simple) ne sont recalculés
        // qu'en passant par l'une de ses méthodes : le profil ICC, remis tel quel.
        let icc = webp.icc_profile();
        webp.set_icc_profile(icc);
        changed = true;
    }
    Ok(changed.then(|| webp.encoder().bytes().to_vec()))
}

// --- HEIF (HEIC) --------------------------------------------------------------------------
//
// Conteneur ISOBMFF : une boîte `meta` décrit les éléments (`iinf`), leurs adresses
// (`iloc`) et leurs liens (`iref`) ; les données sont dans `mdat`. Le XMP est un élément
// `mime` (`application/rdf+xml`), relié à l'image principale par une référence `cdsc`.
// Réécrit à sa place s'il y tient ; sinon ses données vont dans un `mdat` ajouté en fin
// de fichier, et `meta` est reconstruite : comme elle grandit, les adresses des données
// qui la suivent sont décalées d'autant.

/// Type MIME de l'élément XMP.
const HEIF_XMP: &[u8] = b"application/rdf+xml";

/// Une boîte : type, début, début du contenu, fin.
#[derive(Clone, Copy, Debug)]
struct IsoBox {
    kind: [u8; 4],
    start: usize,
    body: usize,
    end: usize,
}

/// Boîtes de `bytes[from..to]`.
fn iso_boxes(bytes: &[u8], from: usize, to: usize) -> Option<Vec<IsoBox>> {
    let mut out = Vec::new();
    let mut at = from;
    while at < to {
        let size = u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as u64;
        let kind: [u8; 4] = bytes.get(at + 4..at + 8)?.try_into().ok()?;
        let (size, header) = match size {
            0 => ((to - at) as u64, 8),
            1 => (
                u64::from_be_bytes(bytes.get(at + 8..at + 16)?.try_into().ok()?),
                16,
            ),
            n => (n, 8),
        };
        let end = at.checked_add(usize::try_from(size).ok()?)?;
        if size < header as u64 || end > to {
            return None;
        }
        out.push(IsoBox {
            kind,
            start: at,
            body: at + header,
            end,
        });
        at = end;
    }
    Some(out)
}

/// Lecteur gros-boutiste dans une tranche.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Self {
        Cursor { bytes, at }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    /// Entier de `size` octets (0, 4 ou 8).
    fn sized(&mut self, size: u8) -> Option<u64> {
        match size {
            0 => Some(0),
            4 => self.u32().map(u64::from),
            8 => Some(u64::from_be_bytes(self.take(8)?.try_into().ok()?)),
            _ => None,
        }
    }
    /// Chaîne terminée par un 0.
    fn cstr(&mut self) -> Option<&'a [u8]> {
        let rest = self.bytes.get(self.at..)?;
        let n = rest.iter().position(|&b| b == 0)?;
        self.at += n + 1;
        Some(&rest[..n])
    }
}

/// Entier gros-boutiste de `size` octets (0, 4 ou 8).
fn put_sized(out: &mut Vec<u8>, size: u8, value: u64) {
    match size {
        4 => out.extend_from_slice(&(value as u32).to_be_bytes()),
        8 => out.extend_from_slice(&value.to_be_bytes()),
        _ => {}
    }
}

/// Un élément de `iloc`.
#[derive(Clone, Debug)]
struct Located {
    id: u32,
    method: u16,
    reference: u16,
    base: u64,
    /// (index, adresse, longueur).
    extents: Vec<(u64, u64, u64)>,
}

/// Boîte `iloc` décodée.
#[derive(Clone, Debug)]
struct Iloc {
    version: u8,
    flags: [u8; 3],
    offset_size: u8,
    length_size: u8,
    base_size: u8,
    index_size: u8,
    items: Vec<Located>,
}

impl Iloc {
    fn parse(body: &[u8]) -> Option<Iloc> {
        let mut c = Cursor::new(body, 0);
        let version = c.u8()?;
        let flags: [u8; 3] = c.take(3)?.try_into().ok()?;
        let a = c.u8()?;
        let b = c.u8()?;
        let (offset_size, length_size, base_size) = (a >> 4, a & 15, b >> 4);
        let index_size = if version > 0 { b & 15 } else { 0 };
        let count = if version < 2 {
            u32::from(c.u16()?)
        } else {
            c.u32()?
        };
        let mut items = Vec::new();
        for _ in 0..count {
            let id = if version < 2 {
                u32::from(c.u16()?)
            } else {
                c.u32()?
            };
            let method = if version > 0 { c.u16()? & 15 } else { 0 };
            let reference = c.u16()?;
            let base = c.sized(base_size)?;
            let n = c.u16()?;
            let mut extents = Vec::new();
            for _ in 0..n {
                let index = if version > 0 && index_size > 0 {
                    c.sized(index_size)?
                } else {
                    0
                };
                extents.push((index, c.sized(offset_size)?, c.sized(length_size)?));
            }
            items.push(Located {
                id,
                method,
                reference,
                base,
                extents,
            });
        }
        Some(Iloc {
            version,
            flags,
            offset_size,
            length_size,
            base_size,
            index_size,
            items,
        })
    }

    /// Boîte complète.
    fn encode(&self) -> Vec<u8> {
        let mut out = vec![0, 0, 0, 0];
        out.extend_from_slice(b"iloc");
        out.push(self.version);
        out.extend_from_slice(&self.flags);
        out.push((self.offset_size << 4) | self.length_size);
        out.push((self.base_size << 4) | self.index_size);
        if self.version < 2 {
            out.extend_from_slice(&(self.items.len() as u16).to_be_bytes());
        } else {
            out.extend_from_slice(&(self.items.len() as u32).to_be_bytes());
        }
        for item in &self.items {
            if self.version < 2 {
                out.extend_from_slice(&(item.id as u16).to_be_bytes());
            } else {
                out.extend_from_slice(&item.id.to_be_bytes());
            }
            if self.version > 0 {
                out.extend_from_slice(&item.method.to_be_bytes());
            }
            out.extend_from_slice(&item.reference.to_be_bytes());
            put_sized(&mut out, self.base_size, item.base);
            out.extend_from_slice(&(item.extents.len() as u16).to_be_bytes());
            for &(index, offset, length) in &item.extents {
                if self.version > 0 && self.index_size > 0 {
                    put_sized(&mut out, self.index_size, index);
                }
                put_sized(&mut out, self.offset_size, offset);
                put_sized(&mut out, self.length_size, length);
            }
        }
        let size = out.len() as u32;
        out[..4].copy_from_slice(&size.to_be_bytes());
        out
    }
}

/// Une entrée `infe` (versions 2 et 3, les seules des HEIF).
#[derive(Clone, Debug)]
struct ItemInfo {
    id: u32,
    kind: [u8; 4],
    content_type: Vec<u8>,
}

/// Entrées de `iinf` ; `None` si une entrée n'est pas de version 2 ou 3.
fn item_infos(bytes: &[u8], iinf: &IsoBox) -> Option<Vec<ItemInfo>> {
    let version = *bytes.get(iinf.body)?;
    let first = iinf.body + 4 + if version == 0 { 2 } else { 4 };
    iso_boxes(bytes, first, iinf.end)?
        .iter()
        .filter(|b| &b.kind == b"infe")
        .map(|b| {
            let mut c = Cursor::new(&bytes[..b.end], b.body);
            let v = c.u8()?;
            c.take(3)?;
            let id = match v {
                2 => u32::from(c.u16()?),
                3 => c.u32()?,
                _ => return None,
            };
            c.u16()?;
            let kind: [u8; 4] = c.take(4)?.try_into().ok()?;
            c.cstr()?;
            let content_type = if &kind == b"mime" {
                c.cstr()?.to_vec()
            } else {
                Vec::new()
            };
            Some(ItemInfo {
                id,
                kind,
                content_type,
            })
        })
        .collect()
}

/// Ce que dit la boîte `meta` : ses enfants, les éléments, leurs adresses, l'image
/// principale.
struct Meta {
    children: Vec<IsoBox>,
    infos: Vec<ItemInfo>,
    iloc: Iloc,
    primary: u32,
}

impl Meta {
    /// `bytes` : le fichier en entier, ou au moins jusqu'à la fin de `meta`.
    fn parse(bytes: &[u8], meta: IsoBox) -> Option<Meta> {
        let children = iso_boxes(bytes, meta.body + 4, meta.end)?;
        let find = |k: &[u8; 4]| children.iter().find(|b| &b.kind == k).copied();
        let infos = item_infos(bytes, &find(b"iinf")?)?;
        let iloc_box = find(b"iloc")?;
        let iloc = Iloc::parse(&bytes[iloc_box.body..iloc_box.end])?;
        let pitm = find(b"pitm")?;
        let mut c = Cursor::new(&bytes[..pitm.end], pitm.body);
        let primary = match c.u8()? {
            0 => {
                c.take(3)?;
                u32::from(c.u16()?)
            }
            _ => {
                c.take(3)?;
                c.u32()?
            }
        };
        Some(Meta {
            children,
            infos,
            iloc,
            primary,
        })
    }

    /// Identifiant de l'élément XMP.
    fn xmp_id(&self) -> Option<u32> {
        self.infos
            .iter()
            .find(|i| &i.kind == b"mime" && i.content_type == HEIF_XMP)
            .map(|i| i.id)
    }

    /// Adresses (dans le fichier) et longueurs des données de l'élément `id`, rangées
    /// dans le fichier (`iloc` méthode 0) ou dans la boîte `idat` (méthode 1).
    fn ranges(&self, bytes_idat: Option<usize>, id: u32) -> Option<Vec<(u64, u64)>> {
        let item = self.iloc.items.iter().find(|i| i.id == id)?;
        let origin = match item.method {
            0 => 0,
            1 => bytes_idat? as u64,
            _ => return None,
        };
        Some(
            item.extents
                .iter()
                .map(|&(_, offset, length)| (origin + item.base + offset, length))
                .collect(),
        )
    }

    /// Début du contenu de la boîte `idat`, s'il y en a une.
    fn idat(&self) -> Option<usize> {
        self.children
            .iter()
            .find(|b| &b.kind == b"idat")
            .map(|b| b.body)
    }
}

/// En-tête d'un fichier HEIF : boîtes de tête parcourues par sauts jusqu'à `meta`, lue en
/// entier, puis les données de l'élément XMP.
fn scan_heif<R: Read + Seek>(r: &mut R) -> io::Result<Found> {
    let mut found = Found::default();
    let mut at = 0u64;
    loop {
        let mut head = [0u8; 16];
        if r.read_exact(&mut head[..8]).is_err() {
            return Ok(found);
        }
        let mut size = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
        let mut header = 8;
        if size == 1 {
            r.read_exact(&mut head[8..16])?;
            size = u64::from_be_bytes(head[8..16].try_into().unwrap_or_default());
            header = 16;
        }
        if size < header || (at == 0 && &head[4..8] != b"ftyp") {
            return Ok(found);
        }
        if &head[4..8] == b"meta" {
            let len = usize::try_from(size).unwrap_or(usize::MAX).min(MAX_XMP);
            let mut meta = vec![0u8; len];
            meta[..header as usize].copy_from_slice(&head[..header as usize]);
            r.read_exact(&mut meta[header as usize..])?;
            let boxes = iso_boxes(&meta, 0, meta.len()).unwrap_or_default();
            let Some(parsed) = boxes.first().and_then(|b| Meta::parse(&meta, *b)) else {
                return Ok(found);
            };
            let Some(ranges) = parsed.xmp_id().and_then(|id| {
                // `idat` est dans `meta` : adresse relative à la copie lue, ramenée au
                // fichier.
                let idat = parsed.idat().map(|i| i + at as usize);
                parsed.ranges(idat, id)
            }) else {
                return Ok(found);
            };
            let mut xmp = Vec::new();
            for (offset, length) in ranges {
                if xmp.len() as u64 + length > MAX_XMP as u64 {
                    return Ok(found);
                }
                r.seek(SeekFrom::Start(offset))?;
                let mut part = vec![0u8; length as usize];
                r.read_exact(&mut part)?;
                xmp.extend_from_slice(&part);
            }
            found.xmp = Some(xmp);
            return Ok(found);
        }
        at += size;
        r.seek(SeekFrom::Start(at))?;
    }
}

/// HEIF mis à jour (voir l'en-tête de cette partie).
fn update_heif(bytes: &[u8], notes: &Notes) -> Result<Option<Vec<u8>>, String> {
    let malformed = || t!("embed-malformed");
    let top = iso_boxes(bytes, 0, bytes.len()).ok_or_else(malformed)?;
    if top.first().map(|b| &b.kind) != Some(b"ftyp") {
        return Err(malformed());
    }
    if top.iter().any(|b| &b.kind == b"moov") {
        // Séquence d'images (adresses aussi dans `moov`) : pas prise en charge.
        return Err(t!("embed-heif-sequence"));
    }
    let meta_box = *top
        .iter()
        .find(|b| &b.kind == b"meta")
        .ok_or_else(malformed)?;
    let meta = Meta::parse(bytes, meta_box).ok_or_else(malformed)?;
    let xmp_id = meta.xmp_id();
    let ranges = xmp_id.and_then(|id| meta.ranges(meta.idat(), id));
    let existing: Option<Vec<u8>> = match &ranges {
        Some(ranges) => {
            let mut xmp = Vec::new();
            for &(offset, length) in ranges {
                let (o, l) = (offset as usize, length as usize);
                xmp.extend_from_slice(bytes.get(o..o + l).ok_or_else(malformed)?);
            }
            Some(xmp)
        }
        None => None,
    };
    let Some(packet) = updated_packet(existing.as_deref(), notes)? else {
        return Ok(None);
    };
    // À sa place, s'il y tient (une seule plage).
    if let Some(ranges) = &ranges
        && let [(offset, length)] = ranges.as_slice()
        && packet.len() <= *length as usize
        && let Some(padded) = pad_packet(&packet, *length as usize)
    {
        let mut out = bytes.to_vec();
        let o = *offset as usize;
        out[o..o + padded.len()].copy_from_slice(&padded);
        return Ok(Some(out));
    }
    // Sinon : `meta` reconstruite, les données du XMP dans un `mdat` en fin de fichier.
    let old_end = meta_box.end as u64;
    let mut iloc = meta.iloc.clone();
    let new_id = xmp_id.unwrap_or_else(|| {
        meta.infos
            .iter()
            .map(|i| i.id)
            .chain(iloc.items.iter().map(|i| i.id))
            .max()
            .unwrap_or(0)
            + 1
    });
    if iloc.version < 2 && new_id > u32::from(u16::MAX) {
        return Err(malformed());
    }
    // Tailles des champs : de quoi loger les adresses (une adresse absente, taille 0,
    // vaut 0 : la garder lisible en passant à 4 ne change rien).
    let big = bytes.len() as u64 + packet.len() as u64 + (1 << 20) > u64::from(u32::MAX);
    let wide = if big { 8 } else { 4 };
    iloc.offset_size = iloc.offset_size.max(wide);
    iloc.length_size = iloc.length_size.max(4);
    iloc.items.retain(|i| i.id != new_id);
    iloc.items.push(Located {
        id: new_id,
        method: 0,
        reference: 0,
        base: 0,
        extents: vec![(0, 0, packet.len() as u64)],
    });
    if iloc.version == 0 && iloc.items.iter().any(|i| i.method != 0) {
        return Err(malformed());
    }
    // Enfants de `meta` : `iinf` et `iref` complétés si l'élément est nouveau, `iloc`
    // remplacée (taille connue d'avance : ses valeurs n'en changent pas la longueur).
    let mut children: Vec<Vec<u8>> = Vec::new();
    let mut has_iref = false;
    for child in &meta.children {
        let raw = &bytes[child.start..child.end];
        match &child.kind {
            b"iloc" => children.push(Vec::new()), // remplie plus bas
            b"iinf" if xmp_id.is_none() => children.push(grow_iinf(raw, new_id)?),
            b"iref" if xmp_id.is_none() => {
                has_iref = true;
                children.push(grow_iref(raw, new_id, meta.primary));
            }
            _ => children.push(raw.to_vec()),
        }
    }
    if xmp_id.is_none() && !has_iref {
        let mut iref = vec![0, 0, 0, 0];
        iref.extend_from_slice(b"iref");
        let wide_ids = new_id > u32::from(u16::MAX) || meta.primary > u32::from(u16::MAX);
        iref.extend_from_slice(&[u8::from(wide_ids), 0, 0, 0]);
        let mut iref = grow_iref(&iref, new_id, meta.primary);
        let size = iref.len() as u32;
        iref[..4].copy_from_slice(&size.to_be_bytes());
        children.push(iref);
    }
    let iloc_at = meta
        .children
        .iter()
        .position(|b| &b.kind == b"iloc")
        .ok_or_else(malformed)?;
    let iloc_len = iloc.encode().len();
    let others: usize = children.iter().map(Vec::len).sum();
    let header = &bytes[meta_box.start..meta_box.body + 4];
    let new_meta_len = 8 + 4 + others + iloc_len;
    let delta = new_meta_len as i64 - (meta_box.end - meta_box.start) as i64;
    // Adresses des données qui suivent `meta` : décalées d'autant.
    let shift = |v: u64| (v as i64 + delta) as u64;
    for item in iloc
        .items
        .iter_mut()
        .filter(|i| i.method == 0 && i.id != new_id)
    {
        if iloc.base_size > 0 && item.base >= old_end {
            item.base = shift(item.base);
        } else {
            for extent in &mut item.extents {
                if item.base + extent.1 >= old_end {
                    extent.1 = shift(extent.1);
                }
            }
        }
    }
    let file_len =
        meta_box.start as u64 + new_meta_len as u64 + (bytes.len() - meta_box.end) as u64;
    if let Some(item) = iloc.items.iter_mut().find(|i| i.id == new_id) {
        item.extents[0].1 = file_len + 8;
    }
    children[iloc_at] = iloc.encode();
    debug_assert_eq!(children[iloc_at].len(), iloc_len);
    let mut out = bytes[..meta_box.start].to_vec();
    out.extend_from_slice(&(new_meta_len as u32).to_be_bytes());
    out.extend_from_slice(b"meta");
    out.extend_from_slice(&header[header.len() - 4..]);
    for child in &children {
        out.extend_from_slice(child);
    }
    out.extend_from_slice(&bytes[meta_box.end..]);
    let mdat_len = u32::try_from(8 + packet.len()).map_err(|_| t!("embed-too-large"))?;
    out.extend_from_slice(&mdat_len.to_be_bytes());
    out.extend_from_slice(b"mdat");
    out.extend_from_slice(&packet);
    Ok(Some(out))
}

/// `iinf` avec une entrée de plus : l'élément XMP `id`.
fn grow_iinf(raw: &[u8], id: u32) -> Result<Vec<u8>, String> {
    let malformed = || t!("embed-malformed");
    let version = *raw.get(8).ok_or_else(malformed)?;
    let mut out = raw.to_vec();
    if version == 0 {
        let n = u16::from_be_bytes([raw[12], raw[13]])
            .checked_add(1)
            .ok_or_else(malformed)?;
        out[12..14].copy_from_slice(&n.to_be_bytes());
    } else {
        let n = u32::from_be_bytes(raw[12..16].try_into().map_err(|_| malformed())?) + 1;
        out[12..16].copy_from_slice(&n.to_be_bytes());
    }
    let mut infe = vec![0, 0, 0, 0];
    infe.extend_from_slice(b"infe");
    if id > u32::from(u16::MAX) {
        infe.extend_from_slice(&[3, 0, 0, 0]);
        infe.extend_from_slice(&id.to_be_bytes());
    } else {
        infe.extend_from_slice(&[2, 0, 0, 0]);
        infe.extend_from_slice(&(id as u16).to_be_bytes());
    }
    infe.extend_from_slice(&[0, 0]);
    infe.extend_from_slice(b"mime");
    infe.push(0);
    infe.extend_from_slice(HEIF_XMP);
    infe.push(0);
    let size = infe.len() as u32;
    infe[..4].copy_from_slice(&size.to_be_bytes());
    out.extend_from_slice(&infe);
    let size = out.len() as u32;
    out[..4].copy_from_slice(&size.to_be_bytes());
    Ok(out)
}

/// `iref` avec une référence de plus : `cdsc` de l'élément `from` vers `to` (le XMP
/// décrit l'image principale).
fn grow_iref(raw: &[u8], from: u32, to: u32) -> Vec<u8> {
    let wide = raw.get(8).is_some_and(|&v| v > 0);
    let mut out = raw.to_vec();
    let mut child = vec![0, 0, 0, 0];
    child.extend_from_slice(b"cdsc");
    if wide {
        child.extend_from_slice(&from.to_be_bytes());
        child.extend_from_slice(&1u16.to_be_bytes());
        child.extend_from_slice(&to.to_be_bytes());
    } else {
        child.extend_from_slice(&(from as u16).to_be_bytes());
        child.extend_from_slice(&1u16.to_be_bytes());
        child.extend_from_slice(&(to as u16).to_be_bytes());
    }
    let size = child.len() as u32;
    child[..4].copy_from_slice(&size.to_be_bytes());
    out.extend_from_slice(&child);
    let size = out.len() as u32;
    out[..4].copy_from_slice(&size.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xmp::{Color, Flag};

    fn notes(stars: u8, flag: Flag, colors: &[Color]) -> Notes {
        let mut n = Notes::default();
        n.set_stars(stars);
        n.flag = flag;
        n.colors = colors.to_vec();
        n
    }

    /// Plus petit JPEG valide qu'`image` sait écrire, avec un EXIF qui porte `rating`.
    fn jpeg(rating: Option<u16>) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbImage::new(4, 4)
            .write_to(&mut io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .unwrap();
        if let Some(r) = rating {
            // Segment EXIF : IFD0 d'une entrée, Rating (SHORT).
            let mut app1 = vec![0xFF, 0xE1, 0, 34];
            app1.extend_from_slice(b"Exif\0\0MM\0\x2A\0\0\0\x08\0\x01\x47\x46\0\x03\0\0\0\x01");
            app1.extend_from_slice(&r.to_be_bytes());
            app1.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
            out.splice(2..2, app1);
        }
        out
    }

    fn image_bytes(format: image::ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbaImage::from_pixel(5, 3, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut io::Cursor::new(&mut out), format)
            .unwrap();
        out
    }

    /// Écrit, relit, et vérifie que l'image se décode toujours à l'identique.
    fn round_trip(name: &str, bytes: &[u8]) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let pixels = image::open(&path).unwrap().to_rgba8();
        assert_eq!(read(&path).unwrap(), None, "{name} : rien au départ");
        let wanted = notes(4, Flag::Accepted, &[Color::Red]);
        assert!(write(&path, &wanted).unwrap(), "{name} : écrit");
        let back = read(&path).unwrap().expect("notes relues");
        assert!(same_notes(&back, &wanted), "{name} : {back:?}");
        assert_eq!(
            image::open(&path).unwrap().to_rgba8(),
            pixels,
            "{name} : image intacte"
        );
        // Deuxième écriture identique : rien ne change ; puis une autre note, à sa place.
        assert!(!write(&path, &wanted).unwrap(), "{name} : rien à changer");
        let size = std::fs::metadata(&path).unwrap().len();
        let other = notes(2, Flag::None, &[]);
        assert!(write(&path, &other).unwrap());
        assert!(same_notes(&read(&path).unwrap().unwrap(), &other));
        let grown = std::fs::metadata(&path).unwrap().len();
        assert!(
            grown <= size + 64,
            "{name} : paquet remplacé, pas ajouté ({size} → {grown})"
        );
        assert!(
            !tmp_path(&path).exists(),
            "{name} : copie de sécurité retirée"
        );
    }

    #[test]
    fn notes_are_written_into_each_format_and_read_back() {
        round_trip("a.jpg", &jpeg(None));
        round_trip("a.png", &image_bytes(image::ImageFormat::Png));
        round_trip("a.webp", &image_bytes(image::ImageFormat::WebP));
        round_trip("a.tif", &image_bytes(image::ImageFormat::Tiff));
    }

    #[test]
    fn an_exif_rating_is_read_and_kept_in_step() {
        // Étoiles posées dans l'Explorateur Windows (EXIF Rating) : lues sans XMP.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.JPG");
        std::fs::write(&path, jpeg(Some(3))).unwrap();
        assert_eq!(read(&path).unwrap().unwrap().stars, 3);
        // Écrites : le XMP les porte, et l'EXIF suit (sinon Windows montrerait l'ancienne).
        write(&path, &notes(5, Flag::None, &[])).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let segs = jpeg_segments(&bytes).unwrap();
        let exif = segs
            .iter()
            .find(|s| bytes[s.1 + 4..s.2].starts_with(JPEG_EXIF))
            .unwrap();
        assert_eq!(tiff_rating(&bytes[exif.1 + 10..exif.2]), Some(5));
        // Rejetée : 0 dans l'EXIF, qui ne connaît pas le rejet.
        write(&path, &notes(0, Flag::Rejected, &[])).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(tiff_rating(&bytes[exif.1 + 10..exif.2]), Some(0));
        assert_eq!(read(&path).unwrap().unwrap().flag, Flag::Rejected);
    }

    #[test]
    fn an_existing_packet_is_completed_not_replaced() {
        // Un paquet d'un autre logiciel (mots-clés) : gardé, nos champs ajoutés.
        let other = "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
            <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
            <rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\
            <dc:subject><rdf:Bag><rdf:li>chien</rdf:li></rdf:Bag></dc:subject>\
            </rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>";
        let mut bytes = jpeg(None);
        let mut seg = vec![0xFF, 0xE1];
        let len = (2 + JPEG_XMP.len() + other.len()) as u16;
        seg.extend_from_slice(&len.to_be_bytes());
        seg.extend_from_slice(JPEG_XMP);
        seg.extend_from_slice(other.as_bytes());
        bytes.splice(2..2, seg);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k.jpg");
        std::fs::write(&path, bytes).unwrap();
        write(&path, &notes(2, Flag::None, &[])).unwrap();
        let found = scan_jpeg(&mut File::open(&path).unwrap()).unwrap();
        let xmp = String::from_utf8(found.xmp.unwrap()).unwrap();
        assert!(xmp.contains("<rdf:li>chien</rdf:li>"), "{xmp}");
        assert_eq!(xmp::parse(&xmp).unwrap().stars, 2);
        let segs = jpeg_segments(&std::fs::read(&path).unwrap()).unwrap();
        let count = segs.iter().filter(|s| s.0 == 0xE1).count();
        assert_eq!(count, 1, "un seul segment XMP");
    }

    #[test]
    fn a_tiff_packet_too_large_for_its_place_moves_to_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.tiff");
        std::fs::write(&path, image_bytes(image::ImageFormat::Tiff)).unwrap();
        let pixels = image::open(&path).unwrap().to_rgba8();
        write(&path, &notes(1, Flag::None, &[])).unwrap();
        // Plus de couleurs : plus long, il ne tient plus à sa place.
        let many = notes(1, Flag::Accepted, &[Color::Red, Color::Green, Color::Blue]);
        write(&path, &many).unwrap();
        let back = read(&path).unwrap().unwrap();
        assert!(same_notes(&back, &many), "{back:?}");
        assert_eq!(image::open(&path).unwrap().to_rgba8(), pixels);
    }

    #[test]
    fn other_files_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("a.RAF");
        std::fs::write(&raw, b"pas une image").unwrap();
        assert_eq!(read(&raw).unwrap(), None);
        assert!(write(&raw, &notes(3, Flag::None, &[])).is_err());
        assert_eq!(
            std::fs::read(&raw).unwrap(),
            b"pas une image",
            "jamais un RAW"
        );
        let broken = dir.path().join("b.jpg");
        std::fs::write(&broken, b"\xFF\xD8abc").unwrap();
        assert!(write(&broken, &notes(3, Flag::None, &[])).is_err());
        assert_eq!(std::fs::read(&broken).unwrap(), b"\xFF\xD8abc");
    }

    /// Données de chaque élément d'un HEIF (sauf le XMP), par identifiant.
    fn heif_items(bytes: &[u8]) -> Vec<(u32, Vec<u8>)> {
        let top = iso_boxes(bytes, 0, bytes.len()).unwrap();
        let meta_box = *top.iter().find(|b| &b.kind == b"meta").unwrap();
        let meta = Meta::parse(bytes, meta_box).unwrap();
        let xmp = meta.xmp_id();
        let mut items: Vec<(u32, Vec<u8>)> = meta
            .iloc
            .items
            .iter()
            .filter(|i| Some(i.id) != xmp)
            .map(|i| {
                let data = meta
                    .ranges(meta.idat(), i.id)
                    .unwrap()
                    .iter()
                    .flat_map(|&(o, l)| bytes[o as usize..(o + l) as usize].to_vec())
                    .collect();
                (i.id, data)
            })
            .collect();
        items.sort();
        items
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn heic_ratings_are_added_and_every_tile_is_kept() {
        // Une grille de 6 tuiles (comme un iPhone), sans XMP : `meta` grandit, les
        // adresses des tuiles sont décalées.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grid.HEIC");
        let original = fixture("grid.heic");
        std::fs::write(&path, &original).unwrap();
        assert_eq!(read(&path).unwrap(), None);
        let wanted = notes(5, Flag::Accepted, &[Color::Red]);
        assert!(write(&path, &wanted).unwrap());
        let written = std::fs::read(&path).unwrap();
        assert_eq!(
            heif_items(&written),
            heif_items(&original),
            "tuiles intactes"
        );
        assert!(same_notes(&read(&path).unwrap().unwrap(), &wanted));
        // Une autre note : à sa place, le fichier ne grandit plus.
        let other = notes(2, Flag::None, &[Color::Red]);
        assert!(write(&path, &other).unwrap());
        assert_eq!(std::fs::read(&path).unwrap().len(), written.len());
        assert!(same_notes(&read(&path).unwrap().unwrap(), &other));
        #[cfg(feature = "heif")]
        {
            let before = crate::flat::decode(dir.path().join("grid.HEIC")).unwrap();
            let copy = dir.path().join("orig.heic");
            std::fs::write(&copy, &original).unwrap();
            let orig = crate::flat::decode(&copy).unwrap();
            assert_eq!(before.data, orig.data, "image décodée à l'identique");
        }
    }

    #[test]
    fn heic_with_a_packet_from_another_program() {
        // XMP posé par ExifTool (Rating 2), dans `mdat` : lu, puis complété.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.heic");
        let original = fixture("xmp.heic");
        std::fs::write(&path, &original).unwrap();
        assert_eq!(read(&path).unwrap().unwrap().stars, 2);
        let wanted = notes(4, Flag::None, &[Color::Green, Color::Blue]);
        assert!(write(&path, &wanted).unwrap());
        assert!(same_notes(&read(&path).unwrap().unwrap(), &wanted));
        assert_eq!(
            heif_items(&std::fs::read(&path).unwrap()),
            heif_items(&original)
        );
    }

    #[test]
    fn padding_goes_inside_the_packet() {
        let p = b"<x/><?xpacket end=\"w\"?>";
        let padded = pad_packet(p, p.len() + 3).unwrap();
        assert_eq!(padded, b"<x/>   <?xpacket end=\"w\"?>");
    }
}
