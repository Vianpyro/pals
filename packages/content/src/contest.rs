//! A contest: one exercise drawn per slot from its pool (ADR-0043)

use std::{
    collections::HashSet,
    fs, io,
    num::NonZeroU32,
    path::Path,
    time::{Duration, SystemTime},
};

use contracts::Id;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{Window, active_release, exercise::exercise_dir};

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
    pub minutes: NonZeroU32,
    // The grade is set by how many of the three are solved: 50, 90, 100
    pub slots: [Slot; 3],
}

#[derive(Debug, Clone, Copy)]
pub enum Sitting<'a> {
    Offering,
    Group(&'a Id),
    Student(&'a str),
}

impl Sitting<'_> {
    // Prefixed: a group and a student with the same name never share a draw
    fn seed(self) -> String {
        match self {
            Sitting::Offering => "offering".to_owned(),
            Sitting::Group(group) => format!("group/{}", group.as_str()),
            Sitting::Student(student) => format!("student/{student}"),
        }
    }
}

impl Contest {
    pub fn drawn(&self, offering: &str, contest: &Id, sitting: Sitting<'_>) -> [&Id; 3] {
        let words = words(&[offering, contest.as_str(), &sitting.seed()]);
        std::array::from_fn(|slot| {
            let pool = &self.slots[slot].pool;
            &pool[(words[slot] % pool.len() as u64) as usize]
        })
    }

    pub fn draw(
        &self,
        offering: &str,
        contest: &Id,
        sitting: Sitting<'_>,
        student: &str,
    ) -> [&Id; 3] {
        let drawn = self.drawn(offering, contest, sitting);
        let order = words(&[offering, contest.as_str(), &sitting.seed(), student])[0];
        ORDERS[(order % 6) as usize].map(|slot| drawn[slot])
    }

    pub fn window(&self, opens_at: SystemTime) -> Window {
        let length = Duration::from_mins(self.minutes.get().into());
        Window {
            opens_at,
            closes_at: Some(opens_at + length),
        }
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
    use crate::{
        Accommodation, ExtraTime, Schedule,
        fixtures::{CONTEST as VALID, course_with_contest as course},
    };
    use std::{assert_matches, time::UNIX_EPOCH};

    fn id(value: &str) -> Id {
        value.parse().unwrap()
    }

    fn read(name: &str, text: &str) -> io::Result<Contest> {
        contest(&course(name, text), &id("weekly"))
    }

    fn names(ids: [&Id; 3]) -> [String; 3] {
        ids.map(|id| id.as_str().to_owned())
    }

    fn drawn(contest: &Contest, group: &str, student: &str) -> [String; 3] {
        let group = id(group);
        names(contest.draw("log200-a26", &id("weekly"), Sitting::Group(&group), student))
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
        assert_kind(contest(&course, &id("other")), io::ErrorKind::NotFound);
    }

    #[test]
    fn two_slots_are_refused() {
        let text = "minutes = 120\n[[slots]]\npool = [\"a\"]\n[[slots]]\npool = [\"b\"]\n";
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

    #[test]
    fn missing_duration_is_refused() {
        let text = VALID.replace("minutes = 120", "");
        assert_kind(
            read("contest-no-minutes", &text),
            io::ErrorKind::InvalidData,
        );
    }

    #[test]
    fn zero_duration_is_refused() {
        let text = VALID.replace("minutes = 120", "minutes = 0");
        assert_kind(
            read("contest-zero-minutes", &text),
            io::ErrorKind::InvalidData,
        );
    }

    #[test]
    fn contest_lasts_its_minutes() {
        let contest = read("contest-window", VALID).unwrap();
        let window = contest.window(UNIX_EPOCH);
        assert_eq!(window.opens_at, UNIX_EPOCH);
        assert_eq!(
            window.closes_at,
            Some(UNIX_EPOCH + Duration::from_mins(120))
        );
    }

    #[test]
    fn half_more_time_ends_an_hour_later() {
        let contest = read("contest-extra-time", VALID).unwrap();
        let schedule = Schedule {
            group: Some(contest.window(UNIX_EPOCH)),
            accommodation: Some(Accommodation {
                extra_time: ExtraTime::Percent(50),
                start_shift: Duration::ZERO,
            }),
            mode_applies_accommodation: true,
            ..Default::default()
        };
        let closes_at = schedule.window().unwrap().closes_at;
        assert_eq!(closes_at, Some(UNIX_EPOCH + Duration::from_mins(180)));
    }

    #[test]
    fn staff_see_a_sitting_in_slot_order() {
        let contest = read("contest-drawn-slots", VALID).unwrap();
        for group in ["01", "02", "03", "04", "05"] {
            let group = id(group);
            let names = names(contest.drawn("log200-a26", &id("weekly"), Sitting::Group(&group)));
            assert!(names[0] == "a" || names[0] == "b");
            assert_eq!(names[1..], ["c", "d"]);
        }
    }

    #[test]
    fn a_student_sees_their_sitting_reordered() {
        let contest = read("contest-draw-shared", VALID).unwrap();
        let group = id("01");
        let sitting = names(contest.drawn("log200-a26", &id("weekly"), Sitting::Group(&group)));
        let orders: HashSet<_> = (0..20)
            .map(|i| drawn(&contest, "01", &format!("student{i}")))
            .inspect(|names| assert_eq!(sorted(names.clone()), sorted(sitting.clone())))
            .collect();
        assert!(orders.len() > 1);
    }

    #[test]
    fn a_group_and_a_student_never_share_a_seed() {
        let alice = id("alice");
        assert_ne!(
            Sitting::Group(&alice).seed(),
            Sitting::Student("alice").seed()
        );
        assert_ne!(
            Sitting::Offering.seed(),
            Sitting::Student("offering").seed()
        );
    }
}
