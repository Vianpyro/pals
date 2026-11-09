//! Grading of choice items: single, true/false, check boxes, dropdown (ADR-0015)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Single,
    Multiple,
}

/// Each choice weight a percentage of the item, negative for a penalised one
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceKey {
    pub selection: Selection,
    pub weights: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerError {
    UnknownChoice(usize),
    Duplicate(usize),
    TooMany,
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
}
