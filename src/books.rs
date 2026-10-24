//! Fonctionnalite "livres" : contenu serveur inscrit dans un livre Minecraft.
//!
//! Le serveur depose des definitions JSON dans `<data_folder>/livres/`.
//! Chaque fichier decrit un livre (titre, auteur, pages). Un joueur le recoit
//! via `/livre <id>`. Les pages supportent les couleurs legacy (`&e`, `&l`...)
//! et les sauts de ligne `\n` (255 caracteres max par page en jeu).

use crate::config;

/// Un livre configurable charge depuis le dossier de donnees.
#[derive(Default, Clone)]
pub struct Book {
    /// Titre affiche du livre.
    pub title: String,
    /// Auteur affiche du livre.
    pub author: String,
    /// Contenu des pages, couleurs legacy autorisees.
    pub pages: Vec<String>,
}

/// Charge toutes les definitions de livres depuis `<data_folder>/livres/`.
/// Retourne une map id -> livre, triee pour un listing deterministe.
pub fn load_books(data_folder: &str) -> std::collections::BTreeMap<String, Book> {
    let mut books = std::collections::BTreeMap::new();
    if data_folder.is_empty() {
        return books;
    }
    let dir = format!("{}/livres", data_folder);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return books;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(book) = parse_book(&content) {
            books.insert(stem.to_string(), book);
        }
    }
    books
}

/// Parse une definition JSON de livre.
fn parse_book(content: &str) -> Option<Book> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(content) else {
        return None;
    };
    let get_str = |key: &str, default: &str| {
        map.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(default)
            .to_string()
    };
    let title = get_str("titre", "Sans titre");
    let author = get_str("auteur", "Serveur");
    let mut pages = Vec::new();
    match map.get("pages") {
        Some(serde_json::Value::Array(list)) => {
            for page in list {
                if let Some(text) = page.as_str() {
                    pages.push(text.to_string());
                }
            }
        }
        Some(serde_json::Value::String(text)) => pages.push(text.clone()),
        _ => {}
    }
    if pages.is_empty() {
        return None;
    }
    Some(Book {
        title,
        author,
        pages,
    })
}

/// Liste des ids de livres disponibles, separees par des virgules.
pub fn list_ids(books: &std::collections::BTreeMap<String, Book>) -> String {
    books.keys().cloned().collect::<Vec<_>>().join(", ")
}

/// Genere le contenu du data component `minecraft:written_book_content`
/// au format attendu par Pumpkin (voir WrittenBookContentImpl::serialize
/// dans pumpkin-protocol/src/codec/data_component.rs) :
/// titre (string reseau), titre filtre (bool), auteur (string reseau),
/// generation (varint), nombre de pages (varint), puis par page
/// le composant texte encode en NBT Java (u8 type id + contenu,
/// sans nom de racine pour >= 1.20.2) suivi d'un bool, et enfin
/// un bool `resolved`.
pub fn written_book_content(book: &Book) -> Vec<u8> {
    let mut out = Vec::new();
    write_string(&mut out, &book.title);
    out.push(0);
    write_string(&mut out, &book.author);
    write_varint(&mut out, 0);
    write_varint(&mut out, book.pages.len() as i32);
    for page in &book.pages {
        write_text_component(&mut out, page);
        out.push(0);
    }
    out.push(1);
    out
}

/// Ecrit une string "reseau" Minecraft : longueur en varint puis UTF-8.
fn write_string(out: &mut Vec<u8>, value: &str) {
    write_varint(out, value.len() as i32);
    out.extend_from_slice(value.as_bytes());
}

/// Ecrit un varint Minecraft (LEB128).
fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

/// Encode une page (string avec eventuels codes couleur legacy) en
/// composant texte NBT Java, comme TextComponent::encode_for_version :
/// `TAG_String` (id 8) + longueur u16 big-endian + cesu8.
fn write_text_component(out: &mut Vec<u8>, text: &str) {
    let converted = convert_legacy(text);
    let java_string = cesu8::to_java_cesu8(&converted);
    let len = java_string.len().min(u16::MAX as usize);
    out.push(8);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(&java_string[..len]);
}

/// Convertit les codes couleur legacy (`&e`, `&l`, `&r`...) en codes section.
fn convert_legacy(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some(code) if code.is_ascii_alphanumeric() => {
                    result.push('\u{a7}');
                    result.push(code.to_ascii_lowercase());
                }
                Some(code) => {
                    result.push('&');
                    result.push(code);
                }
                None => result.push('&'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Indique si la fonctionnalite est activee dans la configuration.
pub fn is_enabled() -> bool {
    crate::with_config(|c| c.enabled(config::FEATURE_BOOKS))
}
