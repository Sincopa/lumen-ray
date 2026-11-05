//! A smaller geosite.dat for Xray that matches exactly the same domains.
//!
//! Xray keeps every domain of a geosite rule in memory for the life of the
//! process, about 190 bytes each. The antifilter category lists ~620k domains,
//! and one in six is a subdomain of another entry in the same category
//! ("a.b.example" next to "b.example"), which the parent already matches.
//! Dropping those saves ~27 MB of Xray working set and ~60 MB of startup peak.
//!
//! An entry is dropped only when it has no attributes (`@cn` filters must see
//! the same entries) and a plain domain entry without attributes in the same
//! category covers it. Regex and keyword entries are kept. Anything this parser
//! does not understand keeps the original file.

use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

/// Smaller categories are copied as they are: nothing to gain.
const MIN_DOMAINS: usize = 256;

const TYPE_DOMAIN: u64 = 2;
const TYPE_FULL: u64 = 3;

fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// One field: number, wire type, payload (length-delimited) and its raw bytes.
struct Field<'a> {
    number: u64,
    payload: &'a [u8],
    raw: &'a [u8],
}

/// The field at `at`, moving past it.
fn next_field<'a>(bytes: &'a [u8], at: &mut usize) -> Option<Field<'a>> {
    let start = *at;
    let key = varint(bytes, at)?;
    let payload = match key & 7 {
        0 => {
            varint(bytes, at)?;
            &bytes[*at..*at]
        }
        1 => {
            *at = at.checked_add(8).filter(|end| *end <= bytes.len())?;
            &bytes[*at..*at]
        }
        2 => {
            let length = usize::try_from(varint(bytes, at)?).ok()?;
            let end = at.checked_add(length).filter(|end| *end <= bytes.len())?;
            let payload = &bytes[*at..end];
            *at = end;
            payload
        }
        5 => {
            *at = at.checked_add(4).filter(|end| *end <= bytes.len())?;
            &bytes[*at..*at]
        }
        _ => return None,
    };
    Some(Field {
        number: key >> 3,
        payload,
        raw: &bytes[start..*at],
    })
}

fn fields(bytes: &[u8]) -> Option<Vec<Field<'_>>> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < bytes.len() {
        out.push(next_field(bytes, &mut at)?);
    }
    Some(out)
}

/// FNV-1a: the keys are short domain names from a verified database, where
/// the default SipHash only costs time.
#[derive(Default)]
struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = hash;
    }
}

/// Plain domain entries of a category, each with the index of its first occurrence.
type Roots<'a> = HashMap<&'a [u8], usize, BuildHasherDefault<Fnv>>;

/// Type and value of a domain entry without attributes or unknown fields.
fn plain_domain(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let mut kind = 0;
    let mut value = None;
    let mut at = 0;
    while at < bytes.len() {
        let key = varint(bytes, &mut at)?;
        match (key >> 3, key & 7) {
            (1, 0) => kind = varint(bytes, &mut at)?,
            (2, 2) => {
                let length = usize::try_from(varint(bytes, &mut at)?).ok()?;
                let end = at.checked_add(length).filter(|end| *end <= bytes.len())?;
                value = Some(&bytes[at..end]);
                at = end;
            }
            // Attributes or anything else: keep the entry as it is.
            _ => return None,
        }
    }
    Some((kind, value?))
}

/// Whether a parent domain (a proper suffix at a label boundary) is listed.
fn covered(value: &[u8], roots: &Roots<'_>) -> bool {
    value
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'.')
        .any(|(dot, _)| roots.contains_key(&value[dot + 1..]))
}

/// The category without redundant entries, or None when nothing is redundant.
/// Two passes over the bytes: the first lists plain domains, the second copies
/// what they do not already cover.
fn compact_site(site: &[u8]) -> Option<Vec<u8>> {
    let mut domains = 0usize;
    let mut at = 0;
    while at < site.len() {
        domains += usize::from(next_field(site, &mut at)?.number == 2);
    }
    if domains < MIN_DOMAINS {
        return None;
    }
    let mut roots = Roots::with_capacity_and_hasher(domains, Default::default());
    let mut index = 0usize;
    at = 0;
    while at < site.len() {
        let field = next_field(site, &mut at)?;
        if field.number != 2 {
            continue;
        }
        if let Some((TYPE_DOMAIN, value)) = plain_domain(field.payload) {
            roots.entry(value).or_insert(index);
        }
        index += 1;
    }
    let mut out = Vec::with_capacity(site.len());
    let mut dropped = 0usize;
    index = 0;
    at = 0;
    while at < site.len() {
        let field = next_field(site, &mut at)?;
        if field.number != 2 {
            out.extend_from_slice(field.raw);
            continue;
        }
        let redundant = match plain_domain(field.payload) {
            // Covered by a parent, or a repeat of an earlier entry.
            Some((TYPE_DOMAIN, value)) => {
                roots.get(value) != Some(&index) || covered(value, &roots)
            }
            Some((TYPE_FULL, value)) => roots.contains_key(value) || covered(value, &roots),
            _ => false,
        };
        index += 1;
        if redundant {
            dropped += 1;
        } else {
            out.extend_from_slice(field.raw);
        }
    }
    (dropped > 0).then_some(out)
}

/// A compacted copy of a geosite.dat, or None when it would not be smaller or
/// the file is not in the expected format.
pub fn compact(geosite: &[u8]) -> Option<Vec<u8>> {
    let entries = fields(geosite)?;
    let mut out = Vec::with_capacity(geosite.len());
    let mut changed = false;
    for entry in entries {
        if entry.number != 1 {
            return None;
        }
        match compact_site(entry.payload) {
            Some(site) => {
                changed = true;
                put_varint(&mut out, (1 << 3) | 2);
                put_varint(&mut out, site.len() as u64);
                out.extend_from_slice(&site);
            }
            None => out.extend_from_slice(entry.raw),
        }
    }
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(kind: u64, value: &str, attribute: bool) -> Vec<u8> {
        let mut entry = Vec::new();
        put_varint(&mut entry, 1 << 3);
        put_varint(&mut entry, kind);
        put_varint(&mut entry, (2 << 3) | 2);
        put_varint(&mut entry, value.len() as u64);
        entry.extend_from_slice(value.as_bytes());
        if attribute {
            // Attribute { key = "ads" }
            let attribute = [0x0a, 0x03, b'a', b'd', b's'];
            put_varint(&mut entry, (3 << 3) | 2);
            put_varint(&mut entry, attribute.len() as u64);
            entry.extend_from_slice(&attribute);
        }
        entry
    }

    fn site(code: &str, domains: &[Vec<u8>]) -> Vec<u8> {
        let mut site = Vec::new();
        put_varint(&mut site, (1 << 3) | 2);
        put_varint(&mut site, code.len() as u64);
        site.extend_from_slice(code.as_bytes());
        for entry in domains {
            put_varint(&mut site, (2 << 3) | 2);
            put_varint(&mut site, entry.len() as u64);
            site.extend_from_slice(entry);
        }
        let mut list = Vec::new();
        put_varint(&mut list, (1 << 3) | 2);
        put_varint(&mut list, site.len() as u64);
        list.extend_from_slice(&site);
        list
    }

    fn values(list: &[u8]) -> Vec<(String, Vec<String>)> {
        fields(list)
            .unwrap()
            .into_iter()
            .map(|entry| {
                let fields = fields(entry.payload).unwrap();
                let code = String::from_utf8(fields[0].payload.to_vec()).unwrap();
                let domains = fields[1..]
                    .iter()
                    .map(|field| {
                        let inner = super::fields(field.payload).unwrap();
                        String::from_utf8(inner[1].payload.to_vec()).unwrap()
                    })
                    .collect();
                (code, domains)
            })
            .collect()
    }

    fn filler() -> Vec<Vec<u8>> {
        (0..MIN_DOMAINS)
            .map(|index| domain(TYPE_DOMAIN, &format!("site{index}.example"), false))
            .collect()
    }

    #[test]
    fn drops_only_entries_a_listed_parent_already_matches() {
        let mut entries = filler();
        entries.extend([
            domain(TYPE_DOMAIN, "blocked.example", false),
            domain(TYPE_DOMAIN, "www.blocked.example", false),
            domain(TYPE_DOMAIN, "a.b.blocked.example", false),
            domain(TYPE_FULL, "blocked.example", false),
            domain(TYPE_FULL, "cdn.blocked.example", false),
            domain(TYPE_DOMAIN, "blocked.example", false),
            // Not covered: a sibling, a suffix that is not a label, a regex.
            domain(TYPE_DOMAIN, "notblocked.example", false),
            domain(TYPE_DOMAIN, "other.example", false),
            domain(1, r"^x\.blocked\.example$", false),
            // Attributes must survive `@attr` filters.
            domain(TYPE_DOMAIN, "ads.blocked.example", true),
        ]);
        let original = site("ANTIFILTER", &entries);
        let compacted = compact(&original).unwrap();
        let (code, kept) = values(&compacted).remove(0);
        assert_eq!(code, "ANTIFILTER");
        let tail: Vec<_> = kept[MIN_DOMAINS..].iter().map(String::as_str).collect();
        assert_eq!(
            tail,
            [
                "blocked.example",
                "notblocked.example",
                "other.example",
                r"^x\.blocked\.example$",
                "ads.blocked.example",
            ]
        );
    }

    #[test]
    fn small_or_clean_databases_stay_as_they_are() {
        let small = site(
            "TINY",
            &[
                domain(TYPE_DOMAIN, "a.example", false),
                domain(TYPE_DOMAIN, "b.a.example", false),
            ],
        );
        assert!(compact(&small).is_none());
        assert!(compact(&site("CLEAN", &filler())).is_none());
        assert!(compact(b"\xff\xff\xff").is_none());
    }

    #[test]
    fn untouched_categories_keep_their_bytes() {
        let mut entries = filler();
        entries.push(domain(TYPE_DOMAIN, "x.site0.example", false));
        let mut list = site("BIG", &entries);
        let other = site("OTHER", &[domain(TYPE_FULL, "keep.example", false)]);
        list.extend_from_slice(&other);
        let compacted = compact(&list).unwrap();
        assert!(compacted.ends_with(&other));
        assert_eq!(values(&compacted)[0].1.len(), MIN_DOMAINS);
    }

    #[test]
    fn bundled_antifilter_shrinks_by_its_redundant_subdomains() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/xray/geosite.dat");
        let Ok(original) = std::fs::read(path) else {
            return;
        };
        let compacted = compact(&original).unwrap();
        assert!(compacted.len() < original.len());
        let count = |list: &[u8], code: &str| {
            fields(list)
                .unwrap()
                .into_iter()
                .find_map(|entry| {
                    let inner = fields(entry.payload)?;
                    (inner.first()?.payload == code.as_bytes())
                        .then(|| inner.iter().filter(|field| field.number == 2).count())
                })
                .unwrap()
        };
        assert!(count(&compacted, "ANTIFILTER") < count(&original, "ANTIFILTER") * 9 / 10);
        // Every category is still there.
        assert_eq!(
            fields(&compacted).unwrap().len(),
            fields(&original).unwrap().len()
        );
    }
}
