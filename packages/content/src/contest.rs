//! A contest: one exercise drawn per slot from its pool (ADR-0043)

use std::{collections::HashSet, fs, io, path::Path};

use contracts::Id;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{active_release, exercise::exercise_dir};

// Every order the three drawn exercises can be shown in
const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub pool: Vec<Id>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contest {
    pub slots: Vec<Slot>,
}

impl Contest {
    pub fn draw(&self, offering: &str, contest: &Id, sitting: &str, student: &str) -> [&Id; 3] {
        let exercises = words(&[offering, contest.as_str(), sitting]);
        let order = words(&[offering, contest.as_str(), sitting, student])[0];

        let pick = |slot: usize| {
            let pool = &self.slots[slot].pool;
            &pool[(exercises[slot] % pool.len() as u64) as usize]
        };
        ORDERS[(order % 6) as usize].map(pick)
    }
}

// Recomputed on every request, never stored: SHA-256, not DefaultHasher, so the draw never changes (ADR-0043)
fn words(parts: &[&str]) -> [u64; 4] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let (chunks, _) = digest.as_chunks::<8>();
    std::array::from_fn(|i| u64::from_le_bytes(chunks[i]))
}

pub fn contest(course_dir: &Path, id: &Id) -> io::Result<Contest> {
    let release = active_release(course_dir)?;
    let file = release
        .join("contests")
        .join(format!("{}.toml", id.as_str()));
    let text = fs::read_to_string(file)?;
    let contest: Contest = toml::from_str(&text).map_err(|error| invalid(&error.to_string()))?;

    // The grade is set by how many of the three are solved: 50, 90, 100
    (contest.slots.len() == 3).ok_or_else(|| invalid("a contest has three slots"))?;
    let mut seen = HashSet::new();
    for slot in &contest.slots {
        (!slot.pool.is_empty()).ok_or_else(|| invalid("a pool is empty"))?;
        for exercise in &slot.pool {
            seen.insert(exercise)
                .ok_or_else(|| invalid("an exercise is in two pools"))?;
            exercise_dir(&release, exercise)?;
        }
    }

    Ok(contest)
}

fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::temp_dir;
    use std::{assert_matches, path::PathBuf};

    const VALID: &str = r#"
[[slots]]
pool = ["a", "b"]
[[slots]]
pool = ["c"]
[[slots]]
pool = ["d"]
"#;

    fn course(name: &str, contest: &str) -> PathBuf {
        let course = temp_dir(name);
        fs::write(course.join("current"), "abc123").unwrap();
        let release = course.join("releases/abc123");
        for exercise in ["a", "b", "c", "d"] {
            fs::create_dir_all(release.join("exercises").join(exercise)).unwrap();
        }
        fs::create_dir_all(release.join("contests")).unwrap();
        fs::write(release.join("contests/weekly.toml"), contest).unwrap();
        course
    }

    fn read(name: &str, text: &str) -> io::Result<Contest> {
        contest(&course(name, text), &"weekly".parse().unwrap())
    }

    fn drawn(contest: &Contest, sitting: &str, student: &str) -> [String; 3] {
        let weekly = "weekly".parse().unwrap();
        contest
            .draw("log200-a26", &weekly, sitting, student)
            .map(|id| id.as_str().to_owned())
    }

    fn sorted(mut names: [String; 3]) -> [String; 3] {
        names.sort();
        names
    }

    #[track_caller]
    fn assert_kind(result: io::Result<Contest>, kind: io::ErrorKind) {
        assert_matches!(result, Err(error) if error.kind() == kind);
    }

    #[test]
    fn three_pools_are_read() {
        let contest = read("contest-valid", VALID).unwrap();
        let first: Vec<&str> = contest.slots[0].pool.iter().map(Id::as_str).collect();
        assert_eq!(first, ["a", "b"]);
    }

    #[test]
    fn missing_contest_is_not_found() {
        let course = course("contest-missing", VALID);
        assert_kind(
            contest(&course, &"other".parse().unwrap()),
            io::ErrorKind::NotFound,
        );
    }

    #[test]
    fn two_slots_are_refused() {
        let text = "[[slots]]\npool = [\"a\"]\n[[slots]]\npool = [\"b\"]\n";
        assert_kind(read("contest-two", text), io::ErrorKind::InvalidData);
    }

    #[test]
    fn empty_pool_is_refused() {
        let text = VALID.replace(r#"["c"]"#, "[]");
        assert_kind(read("contest-empty", &text), io::ErrorKind::InvalidData);
    }

    #[test]
    fn shared_exercise_is_refused() {
        let text = VALID.replace(r#"["c"]"#, r#"["a"]"#);
        assert_kind(read("contest-shared", &text), io::ErrorKind::InvalidData);
    }

    #[test]
    fn unknown_exercise_is_not_found() {
        let text = VALID.replace(r#"["c"]"#, r#"["z"]"#);
        assert_kind(read("contest-unknown", &text), io::ErrorKind::NotFound);
    }

    #[test]
    fn invalid_id_is_refused() {
        let text = VALID.replace(r#"["c"]"#, r#"["A B"]"#);
        assert_kind(read("contest-bad-id", &text), io::ErrorKind::InvalidData);
    }

    #[test]
    fn unknown_field_is_refused() {
        let text = format!("points = 3\n{VALID}");
        assert_kind(read("contest-field", &text), io::ErrorKind::InvalidData);
    }

    #[test]
    fn draw_takes_one_exercise_per_slot() {
        let contest = read("contest-draw-slots", VALID).unwrap();
        for sitting in ["01", "02", "03", "04", "05"] {
            let names = sorted(drawn(&contest, sitting, "alice"));
            assert!(names[0] == "a" || names[0] == "b");
            assert_eq!(names[1..], ["c", "d"]);
        }
    }

    #[test]
    fn a_sitting_shares_its_exercises() {
        let contest = read("contest-draw-shared", VALID).unwrap();
        let alice = drawn(&contest, "01", "alice");
        let orders: HashSet<_> = (0..20)
            .map(|i| drawn(&contest, "01", &format!("student{i}")))
            .inspect(|names| assert_eq!(sorted(names.clone()), sorted(alice.clone())))
            .collect();
        assert!(orders.len() > 1);
    }

    #[test]
    fn sittings_can_draw_differently() {
        let contest = read("contest-draw-sittings", VALID).unwrap();
        let draws: HashSet<_> = (0..20)
            .map(|i| sorted(drawn(&contest, &format!("{i:02}"), "alice")))
            .collect();
        assert!(draws.len() > 1);
    }

    #[test]
    fn draw_never_changes() {
        let contest = read("contest-draw-pinned", VALID).unwrap();
        assert_eq!(drawn(&contest, "01", "alice"), ["a", "d", "c"]);
    }
}
