//! A course's roster, imported from a CSV file when Moodle is not available (ADR-0012)

use std::{error::Error, fmt};

use contracts::Id;
use sqlx::PgPool;

const HEADER: &str = "account,role";

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "course_role", rename_all = "lowercase")]
pub(super) enum CourseRole {
    Student,
    Ta,
    Instructor,
    Coordinator,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Enrollment {
    account: String,
    role: CourseRole,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RosterError {
    Header,
    Line(usize),
    Duplicate(usize),
    Empty,
}

impl fmt::Display for RosterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header => write!(f, "the first line must be {HEADER:?}"),
            Self::Line(line) => write!(
                f,
                "line {line}: expected an account, then student, ta, instructor or coordinator"
            ),
            Self::Duplicate(line) => write!(f, "line {line}: this account is already listed"),
            Self::Empty => write!(f, "no enrollment: an empty roster would remove everyone"),
        }
    }
}

impl Error for RosterError {}

pub(super) fn parse(csv: &str) -> Result<Vec<Enrollment>, RosterError> {
    let csv = csv.strip_prefix('\u{feff}').unwrap_or(csv);
    let mut lines = csv.lines();
    if lines.next() != Some(HEADER) {
        return Err(RosterError::Header);
    }

    let mut roster: Vec<Enrollment> = Vec::new();
    for (index, line) in lines.enumerate() {
        let number = index + 2;
        let enrollment = enrollment(line).ok_or(RosterError::Line(number))?;
        if roster
            .iter()
            .any(|known| known.account == enrollment.account)
        {
            return Err(RosterError::Duplicate(number));
        }

        roster.push(enrollment);
    }

    if roster.is_empty() {
        return Err(RosterError::Empty);
    }

    Ok(roster)
}

fn enrollment(line: &str) -> Option<Enrollment> {
    let (account, role) = line.split_once(',')?;
    if account.is_empty() || account.contains(char::is_whitespace) {
        return None;
    }

    let role = match role {
        "student" => CourseRole::Student,
        "ta" => CourseRole::Ta,
        "instructor" => CourseRole::Instructor,
        "coordinator" => CourseRole::Coordinator,
        _ => return None,
    };

    Some(Enrollment {
        account: account.to_owned(),
        role,
    })
}

pub(super) async fn replace(pool: &PgPool, course: &Id, roster: &[Enrollment]) -> sqlx::Result<()> {
    let mut transaction = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM course_roles WHERE course = $1::text",
        course.as_str()
    )
    .execute(&mut *transaction)
    .await?;

    for enrollment in roster {
        sqlx::query!(
            "INSERT INTO course_roles (course, account, role) VALUES ($1::text, $2, $3)",
            course.as_str(),
            enrollment.account,
            enrollment.role as CourseRole,
        )
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enrolled(account: &str, role: CourseRole) -> Enrollment {
        Enrollment {
            account: account.to_owned(),
            role,
        }
    }

    #[test]
    fn reads_each_enrollment() {
        let roster = parse("account,role\netudiant1,student\nenseignant1,instructor\n");

        assert_eq!(
            roster,
            Ok(vec![
                enrolled("etudiant1", CourseRole::Student),
                enrolled("enseignant1", CourseRole::Instructor),
            ])
        );
    }

    #[test]
    fn reads_a_file_saved_by_excel() {
        let roster = parse("\u{feff}account,role\r\netudiant1,ta\r\n");

        assert_eq!(roster, Ok(vec![enrolled("etudiant1", CourseRole::Ta)]));
    }

    #[test]
    fn refuses_another_header() {
        assert_eq!(
            parse("user,role\netudiant1,student\n"),
            Err(RosterError::Header)
        );
        assert_eq!(parse(""), Err(RosterError::Header));
    }

    #[test]
    fn refuses_a_malformed_line_and_names_it() {
        let lines = [
            "etudiant1",
            "etudiant1,admin",
            ",student",
            " etudiant1,student",
            "etudiant1,student,ta",
            "",
        ];
        for line in lines {
            let csv = format!("account,role\netudiant2,student\n{line}\n");

            assert_eq!(parse(&csv), Err(RosterError::Line(3)), "{line:?}");
        }
    }

    #[test]
    fn refuses_an_account_listed_twice() {
        let roster = parse("account,role\netudiant1,student\netudiant1,ta\n");

        assert_eq!(roster, Err(RosterError::Duplicate(3)));
    }

    #[test]
    fn refuses_an_empty_roster() {
        assert_eq!(parse("account,role\n"), Err(RosterError::Empty));
    }

    #[test]
    fn tells_the_operator_what_to_fix() {
        let messages = [
            RosterError::Header,
            RosterError::Line(4),
            RosterError::Duplicate(5),
            RosterError::Empty,
        ]
        .map(|error| error.to_string());

        assert_eq!(
            messages,
            [
                r#"the first line must be "account,role""#,
                "line 4: expected an account, then student, ta, instructor or coordinator",
                "line 5: this account is already listed",
                "no enrollment: an empty roster would remove everyone",
            ]
        );
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PostgreSQL"]
    async fn replace_leaves_exactly_the_roster(pool: PgPool) {
        sqlx::query(
            "INSERT INTO course_roles VALUES
               ('log200', 'parti', 'student'),
               ('log200', 'etudiant1', 'student'),
               ('log121', 'parti', 'student')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let course: Id = "log200".parse().unwrap();
        let roster = parse("account,role\netudiant1,ta\netudiant2,student\n").unwrap();

        // A second import of the same roster changes nothing
        for _ in 0..2 {
            replace(&pool, &course, &roster).await.unwrap();
        }

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT course::text, account, role::text FROM course_roles ORDER BY course, account",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected = [
            ("log121", "parti", "student"),
            ("log200", "etudiant1", "ta"),
            ("log200", "etudiant2", "student"),
        ]
        .map(|(course, account, role)| (course.to_owned(), account.to_owned(), role.to_owned()));
        assert_eq!(rows, expected);
    }
}
