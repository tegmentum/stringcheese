//! # Phonetic keys for name matching
//!
//! Soundex (NARA 1918), NYSIIS (Taft 1970), and Double Metaphone
//! (Philips 1999) encoded over the same small name list. Two names
//! that share a phonetic key sound alike under the encoder's model,
//! even when spelling differs.
//!
//! Run: `cargo run --example phonetic_encoders -p stringcheese`

use stringcheese::phonetic::{DoubleMetaphone, Nysiis, Soundex};

fn main() {
    let names = [
        "Robert", "Rupert", "Rubin", "Ashcroft", "Ashcraft", "Kaitlin", "Catelyn", "Xavier",
    ];

    let dm = DoubleMetaphone::full();

    println!(
        "{:<10}  {:<7}  {:<7}  double-metaphone",
        "name", "soundex", "nysiis"
    );
    println!(
        "{:<10}  {:<7}  {:<7}  ----------------",
        "----", "-------", "------"
    );
    for name in names {
        let sx = Soundex::encode(name);
        let ny = Nysiis::encode(name);
        let key = dm.encode(name);
        let dm_display = match &key.alternate {
            Some(alt) if alt != &key.primary => format!("{} / {}", key.primary, alt),
            _ => key.primary.clone(),
        };
        println!("{name:<10}  {sx:<7}  {ny:<7}  {dm_display}");
    }
}
