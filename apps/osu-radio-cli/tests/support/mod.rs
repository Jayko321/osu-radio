use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = env::temp_dir().join(format!(
            "osu-radio-cli-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join(".env"), "SQLITE_DATABASE_URL=library.sqlite\n").unwrap();
        Self { root }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_osu-radio-cli"));
        command.current_dir(&self.root).env_clear();
        for name in ["SystemRoot", "PATH", "LLVM_PROFILE_FILE"] {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    pub fn install(&self, name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let root = self.root.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("osu!.db"), stable_database(entries)).unwrap();
        for (folder, _) in entries {
            let directory = root.join("Songs").join(folder);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("audio.mp3"), b"source audio stays intact").unwrap();
            fs::write(
                directory.join("map.osu"),
                "[Events]\n0,0,\"cover.jpg\",0,0\n",
            )
            .unwrap();
            fs::write(directory.join("cover.jpg"), b"source artwork stays intact").unwrap();
        }
        root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn stdout(output: &Output) -> String {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout.clone()).unwrap()
}

pub fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).unwrap()
}

pub fn failure(output: &Output, message: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{output:?}"
    );
}

// The single current-layout fixture mirrors stable/tests.rs; this is not a general osu! writer.
fn stable_database(entries: &[(&str, &str)]) -> Vec<u8> {
    fn integer(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend(value.to_le_bytes());
    }
    fn string(bytes: &mut Vec<u8>, value: &str) {
        assert!(value.len() < 128, "fixture uses single-byte string lengths");
        bytes.extend([0x0b, u8::try_from(value.len()).unwrap()]);
        bytes.extend(value.as_bytes());
    }

    let mut bytes = Vec::new();
    integer(&mut bytes, 20_250_108);
    integer(&mut bytes, i32::try_from(entries.len()).unwrap());
    bytes.push(1);
    bytes.extend([0; 8]);
    string(&mut bytes, "Fixture");
    integer(&mut bytes, i32::try_from(entries.len()).unwrap());
    for (index, (folder, difficulty)) in entries.iter().enumerate() {
        for value in [
            "Artist",
            "歌手",
            "Song",
            "曲",
            "Mapper",
            difficulty,
            "audio.mp3",
        ] {
            string(&mut bytes, value);
        }
        string(&mut bytes, &format!("{index:032x}"));
        string(&mut bytes, "map.osu");
        bytes.push(4);
        bytes.extend([0; 14]);
        for _ in 0..4 {
            bytes.extend(5_f32.to_le_bytes());
        }
        bytes.extend(1.4_f64.to_le_bytes());
        bytes.extend([0; 16]); // Four empty star-rating arrays.
        integer(&mut bytes, 15);
        integer(&mut bytes, 20_000);
        integer(&mut bytes, 1234);
        integer(&mut bytes, 1);
        bytes.extend(500_f64.to_le_bytes());
        bytes.extend(0_f64.to_le_bytes());
        bytes.push(1);
        integer(&mut bytes, i32::try_from(index).unwrap());
        integer(&mut bytes, 123);
        integer(&mut bytes, -1);
        bytes.extend([0; 6]);
        bytes.extend(0.7_f32.to_le_bytes());
        bytes.push(0);
        string(&mut bytes, "Game");
        string(&mut bytes, "TAG two");
        bytes.extend([0; 2]);
        string(&mut bytes, "");
        bytes.push(1);
        bytes.extend([0; 9]);
        string(&mut bytes, folder);
        bytes.extend([0; 13]);
        integer(&mut bytes, 0);
        bytes.push(10);
    }
    integer(&mut bytes, 1);
    bytes
}
