//! Every language pack against the same exercise (ADR-0013, V-0013)

use std::{fs, path::Path, time::Duration};

use judge::{Verdict, judge_cases, read_cases, read_packs};

const TIMEOUT: Duration = Duration::from_secs(10);

const EXPECTED: [(&str, Verdict); 4] = [
    ("passed", Verdict::Passed),
    ("wrong-answer", Verdict::WrongAnswer),
    ("crashed", Verdict::Crashed),
    ("timed-out", Verdict::TimedOut),
];

#[tokio::test]
#[ignore = "needs Docker with runsc"]
async fn every_pack_gives_the_expected_verdict() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let languages = root.join("packs/languages");
    let all = read_cases(&root.join("examples/content/exercises/sum/assessment")).unwrap();

    for (id, pack) in read_packs(&languages).unwrap() {
        for (folder, expected) in EXPECTED {
            let cases = if expected == Verdict::TimedOut {
                &all[..1]
            } else {
                &all[..]
            };

            let dir = languages.join(id.as_str()).join("conformance").join(folder);
            let sources: Vec<_> = fs::read_dir(&dir)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();

            assert!(!sources.is_empty(), "{}: nothing to run", dir.display());

            for path in sources {
                let source = fs::read_to_string(&path).unwrap();
                let name = format!("judge-conformance-{}-{folder}", id.as_str());
                let verdicts = judge_cases(&name, &pack, &source, cases, TIMEOUT)
                    .await
                    .unwrap();

                assert_eq!(verdicts, vec![expected; cases.len()], "{}", path.display());
            }
        }
    }
}
