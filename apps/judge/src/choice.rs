//! Grading of choice items: single, true/false, check boxes, dropdown (ADR-0015)

use std::{fs, io, path::Path};

use serde::Deserialize;

use crate::errors::{invalid, located};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Selection {
    Single,
    Multiple,
}

/// Each choice weight a percentage of the item, negative for a penalised one
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceKey {
    pub selection: Selection,
    pub weights: Vec<i32>,
}

pub fn read_choice_key(assessment: &Path) -> io::Result<ChoiceKey> {
    let file = assessment.join("choice.toml");
    let text = fs::read_to_string(&file).map_err(|error| located(&file, error))?;
    let key: ChoiceKey =
        toml::from_str(&text).map_err(|error| invalid(&file, &error.to_string()))?;
    (key.weights.len() >= 2).ok_or_else(|| invalid(&file, "fewer than two choices"))?;
    key.weights
        .iter()
        .all(|weight| (-100..=100).contains(weight))
        .ok_or_else(|| invalid(&file, "a weight is outside -100..=100"))?;
    (best_score(&key) == 100)
        .ok_or_else(|| invalid(&file, "the best answer does not score 100"))?;
    Ok(key)
}

fn best_score(key: &ChoiceKey) -> i32 {
    match key.selection {
        Selection::Single => key.weights.iter().copied().max().unwrap_or(0),
        Selection::Multiple => key.weights.iter().filter(|&&weight| weight > 0).sum(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerError {
    Malformed,
    UnknownChoice(usize),
    Duplicate(usize),
    TooMany,
}

pub fn read_answer(source: &str) -> Result<Vec<usize>, AnswerError> {
    if source.is_empty() {
        return Ok(Vec::new());
    }
    source.split(',').map(choice_index).collect()
}

fn choice_index(text: &str) -> Result<usize, AnswerError> {
    text.bytes()
        .all(|byte| byte.is_ascii_digit())
        .ok_or(AnswerError::Malformed)?;
    text.parse().map_err(|_| AnswerError::Malformed)
}

pub fn grade(key: &ChoiceKey, selected: &[usize]) -> Result<u8, AnswerError> {
    if key.selection == Selection::Single && selected.len() > 1 {
        return Err(AnswerError::TooMany);
    }
    let mut seen = vec![false; key.weights.len()];
    let mut total = 0;
    for &choice in selected {
        let already = seen
            .get_mut(choice)
            .ok_or(AnswerError::UnknownChoice(choice))?;
        if *already {
            return Err(AnswerError::Duplicate(choice));
        }
        *already = true;
        total += key.weights[choice];
    }

    Ok(total.clamp(0, 100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{assert_matches, path::PathBuf};

    fn assessment(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("judge-test-choice-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("choice.toml"), text).unwrap();
        dir
    }

    #[track_caller]
    fn assert_refused(name: &str, text: &str) {
        let result = read_choice_key(&assessment(name, text));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
    }

    #[test]
    fn key_is_read() {
        let dir = assessment(
            "valid",
            "selection = \"multiple\"\nweights = [50, 50, -100]",
        );
        assert_eq!(read_choice_key(&dir).unwrap(), multiple(&[50, 50, -100]));
    }

    #[test]
    fn missing_key_is_not_found() {
        let dir = std::env::temp_dir().join("judge-test-choice-missing");
        let _ = fs::remove_dir_all(&dir);
        assert_matches!(read_choice_key(&dir), Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[test]
    fn unknown_field_is_refused() {
        assert_refused(
            "unknown",
            "selection = \"single\"\nweights = [0, 100]\ncorrect = 1",
        );
    }

    #[test]
    fn selection_is_lowercase() {
        assert_refused("case", "selection = \"Single\"\nweights = [0, 100]");
    }

    #[test]
    fn one_choice_is_refused() {
        assert_refused("one", "selection = \"single\"\nweights = [100]");
    }

    #[test]
    fn weight_out_of_range_is_refused() {
        assert_refused("range", "selection = \"single\"\nweights = [100, -150]");
    }

    #[test]
    fn single_without_a_full_choice_is_refused() {
        assert_refused("single", "selection = \"single\"\nweights = [50, 50]");
    }

    #[test]
    fn multiple_not_summing_to_full_is_refused() {
        assert_refused("multiple", "selection = \"multiple\"\nweights = [50, 60]");
    }

    fn single(weights: &[i32]) -> ChoiceKey {
        ChoiceKey {
            selection: Selection::Single,
            weights: weights.to_vec(),
        }
    }

    fn multiple(weights: &[i32]) -> ChoiceKey {
        ChoiceKey {
            selection: Selection::Multiple,
            weights: weights.to_vec(),
        }
    }

    #[test]
    fn single_right_choice_scores_full() {
        assert_eq!(grade(&single(&[0, 100, 0]), &[1]), Ok(100));
    }

    #[test]
    fn single_wrong_choice_scores_nothing() {
        assert_eq!(grade(&single(&[0, 100, 0]), &[2]), Ok(0));
    }

    #[test]
    fn single_refuses_two_choices() {
        assert_eq!(
            grade(&single(&[0, 100, 0]), &[0, 1]),
            Err(AnswerError::TooMany)
        )
    }

    #[test]
    fn multiple_gives_partial_credit() {
        assert_eq!(grade(&multiple(&[50, 50, -100]), &[0]), Ok(50));
    }

    #[test]
    fn penalty_never_goes_below_zero() {
        assert_eq!(grade(&multiple(&[50, 50, -100]), &[0, 2]), Ok(0));
    }

    #[test]
    fn checking_every_box_does_not_score_full() {
        assert_eq!(grade(&multiple(&[50, 50, -50]), &[0, 1, 2]), Ok(50));
    }

    #[test]
    fn unknown_choice_is_refused() {
        assert_eq!(
            grade(&multiple(&[50, 50]), &[2]),
            Err(AnswerError::UnknownChoice(2))
        );
    }

    #[test]
    fn duplicate_choice_is_refused() {
        assert_eq!(
            grade(&multiple(&[50, 50]), &[0, 0]),
            Err(AnswerError::Duplicate(0))
        );
    }

    #[test]
    fn no_choice_scores_nothing() {
        assert_eq!(grade(&multiple(&[50, 50]), &[]), Ok(0));
    }

    #[test]
    fn answer_lists_its_choices() {
        assert_eq!(read_answer("0,2"), Ok(vec![0, 2]));
    }

    #[test]
    fn empty_answer_selects_nothing() {
        assert_eq!(read_answer(""), Ok(vec![]));
    }

    #[test]
    fn malformed_answer_is_refused() {
        for source in ["0,", ",1", "1,,2", "a", "-1", " 1", "+1"] {
            assert_eq!(
                read_answer(source),
                Err(AnswerError::Malformed),
                "{source:?}"
            );
        }
    }
}
