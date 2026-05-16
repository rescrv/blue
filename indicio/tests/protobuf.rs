#![cfg(feature = "prototk")]

use std::fs::{create_dir, read, read_dir, remove_dir_all};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use buffertk::{Unpackable, stack_pack};
use indicio::{Clue, ClueFrame, ClueVector, Emitter, INFO, ProtobufEmitter, value};

fn temp_prefix(name: &str) -> (PathBuf, PathBuf) {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("indicio-{name}-{}-{timestamp}", std::process::id()));
    create_dir(&root).unwrap();
    let prefix = root.join("clues");
    (root, prefix)
}

fn read_clues(root: &Path) -> Vec<Clue> {
    let mut paths = read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    let mut clues = Vec::new();
    for path in paths {
        let buf = read(path).unwrap();
        let mut vector = ClueVector::unpack(&buf).unwrap().0;
        clues.append(&mut vector.clues);
    }
    clues
}

#[test]
fn concatenated_frames_decode_as_clue_vector() {
    let clues = vec![
        Clue {
            file: "file-a".to_string(),
            line: 10,
            level: INFO,
            timestamp: 100,
            value: value!({ event: "a" }),
        },
        Clue {
            file: "file-b".to_string(),
            line: 20,
            level: INFO,
            timestamp: 200,
            value: value!({ event: "b" }),
        },
    ];
    let mut buf = Vec::new();
    for clue in clues.iter().cloned() {
        stack_pack(&ClueFrame { clue }).append_to_vec(&mut buf);
    }

    assert_eq!(
        ClueVector {
            clues: clues.clone(),
        },
        ClueVector::unpack(&buf).unwrap().0
    );
}

#[test]
fn emitter_rolls_files_without_changing_clue_vector_format() {
    let (root, prefix) = temp_prefix("rolls");
    {
        let emitter = ProtobufEmitter::new(&prefix, 1).unwrap();
        emitter.emit("file-a", 10, INFO, value!({ event: "a" }));
        emitter.flush();
        emitter.emit("file-b", 20, INFO, value!({ event: "b" }));
        emitter.flush();
    }

    let file_count = read_dir(&root).unwrap().count();
    let mut clues = read_clues(&root);
    assert_eq!(2, file_count);
    assert_eq!(2, clues.len());
    assert!(clues[0].timestamp < clues[1].timestamp);
    for clue in clues.iter_mut() {
        clue.timestamp = 0;
    }
    assert_eq!(
        vec![
            Clue {
                file: "file-a".to_string(),
                line: 10,
                level: INFO,
                timestamp: 0,
                value: value!({ event: "a" }),
            },
            Clue {
                file: "file-b".to_string(),
                line: 20,
                level: INFO,
                timestamp: 0,
                value: value!({ event: "b" }),
            },
        ],
        clues
    );

    remove_dir_all(root).unwrap();
}

#[test]
fn emitter_rejects_zero_rollover_target() {
    let (root, prefix) = temp_prefix("zero-target");
    let error = match ProtobufEmitter::new(&prefix, 0) {
        Ok(_) => panic!("zero rollover target should be rejected"),
        Err(error) => error,
    };

    assert_eq!(std::io::ErrorKind::InvalidInput, error.kind());
    assert_eq!(Vec::<Clue>::new(), read_clues(&root));

    remove_dir_all(root).unwrap();
}
