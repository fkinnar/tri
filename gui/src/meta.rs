//! Lecture des métadonnées ([`tri::metadata`]) dans un fil à part : quelques ms en local,
//! mais un disque lent ou un NAS ne doit pas figer l'interface. L'interface demande, puis
//! récupère les résultats à chaque image.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use tri::media::MediaFile;
use tri::metadata::{self, Metadata};

/// Résultat d'une lecture : les métadonnées, ou le message d'erreur.
pub type Read = Result<Arc<Metadata>, String>;

/// Lecteur de métadonnées en fond, avec les résultats déjà reçus.
pub struct MetaLoader {
    requests: Sender<MediaFile>,
    results: Receiver<(PathBuf, Read)>,
    known: HashMap<PathBuf, Read>,
    pending: HashSet<PathBuf>,
}

impl MetaLoader {
    /// Démarre le fil de lecture ; `ctx` est réveillé à chaque résultat.
    pub fn new(ctx: egui::Context) -> MetaLoader {
        let (requests, inbox) = mpsc::channel::<MediaFile>();
        let (outbox, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("tri-metadata".into())
            .spawn(move || {
                for file in inbox {
                    let read = metadata::read(&file)
                        .map(Arc::new)
                        .map_err(|e| e.to_string());
                    if outbox.send((file.path, read)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("fil de lecture des métadonnées");
        MetaLoader {
            requests,
            results,
            known: HashMap::new(),
            pending: HashSet::new(),
        }
    }

    /// Métadonnées de `file` si elles sont lues ; sinon demande la lecture (une fois) et
    /// renvoie `None`.
    pub fn get(&mut self, file: &MediaFile) -> Option<&Read> {
        for (path, read) in self.results.try_iter() {
            self.pending.remove(&path);
            self.known.insert(path, read);
        }
        if !self.known.contains_key(&file.path) && self.pending.insert(file.path.clone()) {
            let _ = self.requests.send(file.clone());
        }
        self.known.get(&file.path)
    }

    /// Oublie tout (changement de dossier, fichiers modifiés).
    pub fn clear(&mut self) {
        self.known.clear();
    }
}
