//! Randomised robustness tests for every parser that reads untrusted input:
//! playlists (M3U/PLS/XSPF, including files on disk), LRC lyrics, ffprobe
//! JSON, smart-playlist rules and tag extraction. Deterministic (fixed seeds)
//! so failures reproduce; each target gets tens of thousands of inputs, mixing
//! raw random bytes with mutations of valid samples. The property is simply
//! "never panics, never hangs".

use crate::{db, library, probe, sources};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const TOKENS: &[&str] = &[
    "#EXTM3U", "#EXTINF:", "\n", "\r\n", "[playlist]", "File1=", "File99999999999999999999=", "=", "<location>", "</location>",
    "<track>", "&amp;", "&", "&#", "%", "%2", "%ZZ", "%C3%BC", "file://", "file:///C:/", "file:////", "http://", "..\\", "../",
    "C:\\", "\\\\?\\", "\u{feff}", "\u{0}", "é", "日本", "[", "]", "[00:", "[99:99.99]", "[offset:", "[-1:-1]", ":", ".", "-",
    "9999999999999999999", "NaN", "{", "}", "\"", "streams", "format", "tags", "duration", "1e309", "-0", "null",
];

fn mutate(rng: &mut Rng, seed: &str) -> String {
    let mut s: Vec<char> = seed.chars().collect();
    for _ in 0..1 + rng.below(8) {
        match rng.below(4) {
            0 if !s.is_empty() => {
                let i = rng.below(s.len());
                s.remove(i);
            }
            1 => {
                let t = TOKENS[rng.below(TOKENS.len())];
                let i = rng.below(s.len() + 1);
                for (k, c) in t.chars().enumerate() {
                    s.insert(i + k, c);
                }
            }
            2 if !s.is_empty() => {
                let i = rng.below(s.len());
                s[i] = char::from_u32(rng.below(0x3000) as u32).unwrap_or('x');
            }
            _ => {
                let i = rng.below(s.len() + 1);
                let j = (i + rng.below(16)).min(s.len());
                let dup: Vec<char> = s[i..j].to_vec();
                for (k, c) in dup.into_iter().enumerate() {
                    s.insert(j + k, c);
                }
            }
        }
    }
    s.into_iter().collect()
}

fn random_bytes(rng: &mut Rng) -> Vec<u8> {
    (0..rng.below(512)).map(|_| rng.next() as u8).collect()
}

fn inputs(seed: u64, samples: &[&str], n: usize) -> impl Iterator<Item = String> {
    let mut rng = Rng(seed);
    let samples: Vec<String> = samples.iter().map(|s| s.to_string()).collect();
    (0..n).map(move |i| {
        if i % 4 == 0 {
            String::from_utf8_lossy(&random_bytes(&mut rng)).into_owned()
        } else {
            let s = samples[rng.below(samples.len())].clone();
            mutate(&mut rng, &s)
        }
    })
}

#[test]
fn playlist_parsers_never_panic() {
    let samples = [
        "#EXTM3U\n#EXTINF:3,A\na.mp3\nhttp://h/s.m3u8\n",
        "[playlist]\nFile1=a.flac\nTitle1=A\nFile2=../b.flac\nNumberOfEntries=2\n",
        r#"<playlist><trackList><track><location>file:///C:/M%C3%BCsic/a%20b.mp3</location></track></trackList></playlist>"#,
    ];
    for t in inputs(0x5eed_0001, &samples, 30_000) {
        let _ = sources::parse_m3u(&t);
        let _ = sources::parse_pls(&t);
        let _ = sources::parse_xspf(&t);
    }
}

#[test]
fn playlist_files_on_disk_never_panic() {
    // Exercises path resolution (relative, file://, UNC, percent-decoding) end to end.
    let dir = std::env::temp_dir().join(format!("halcyon-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let samples = ["#EXTM3U\n../x.mp3\nfile:///C:/a%20b.mp3\nfile://server/share/c.mp3\n", "[playlist]\nFile1=file:///%E6%97%A5.mp3\n"];
    for (i, t) in inputs(0x5eed_0002, &samples, 3_000).enumerate() {
        let f = dir.join(["a.m3u", "b.m3u8", "c.pls", "d.xspf"][i % 4]);
        std::fs::write(&f, t.as_bytes()).unwrap();
        let _ = sources::read_playlist(&f);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn lrc_parser_never_panics() {
    let samples = ["[ti:x]\n[offset:250]\n[00:01.00][00:10.50]Chorus\n[00:05:25]Verse\n", "[01:02.3]a\n[1:2]b"];
    for t in inputs(0x5eed_0003, &samples, 30_000) {
        let v = library::parse_lrc(&t);
        assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "lyrics must stay sorted");
    }
}

#[test]
fn ffprobe_json_and_tag_extraction_never_panic() {
    let samples = [
        r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"opus","channels":2,"tags":{"ARTIST":"Ana","TRACKNUMBER":"3/12","DATE":"2019"}}],"format":{"duration":"61.5","tags":{"title":"S"}}}"#,
        r#"{"streams":[{"index":1,"codec_type":"video","width":3840,"height":2160,"disposition":{"attached_pic":1}}],"format":{},"stream_groups":[{"type":"IAMF Audio Element"}]}"#,
    ];
    let mut parsed = 0;
    for t in inputs(0x5eed_0004, &samples, 30_000) {
        if let Ok(p) = serde_json::from_str::<probe::ProbeResult>(&t) {
            parsed += 1;
            let _ = library::row_from_probe("x", &p, 0, 0);
        }
    }
    assert!(parsed > 1000, "mutations should still produce many valid documents ({parsed})");
}

#[test]
fn smart_rules_never_reach_sql_unescaped() {
    let d = db::Db::memory();
    let mut rng = Rng(0x5eed_0005);
    let fields = ["title", "artist", "genre", "plays", "year", "favorite", "added_days", "bogus", "title; DROP TABLE media"];
    let ops = ["contains", "not_contains", "is", "is_not", "gt", "lt", "like", ""];
    let values = inputs(0x5eed_0006, &["x", "50%", "a_b", "'; DROP TABLE media; --", "\\", "1"], 5_000).collect::<Vec<_>>();
    for v in values {
        let rules: Vec<db::Rule> = (0..1 + rng.below(3))
            .map(|_| db::Rule { field: fields[rng.below(fields.len())].into(), op: ops[rng.below(ops.len())].into(), value: v.clone() })
            .collect();
        let r = db::SmartRules { match_: if rng.below(2) == 0 { "all".into() } else { "any".into() }, rules, limit: Some(rng.below(10) as i64 - 2), sort: Some("random".into()) };
        let _ = d.smart_items(&r); // Err for invalid rules is fine; a panic or SQL error from injection is not
    }
    // The schema is intact.
    assert!(d.list_media(None, None, false, None).is_ok());
}
