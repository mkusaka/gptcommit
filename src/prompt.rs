use tera::{Context, Error};

use std::collections::HashMap;
use tera::Tera;

pub fn format_prompt(prompt: &str, map: HashMap<&str, &str>) -> Result<String, Error> {
    let context = Context::from_serialize(map)?;

    Tera::one_off(prompt, &context, false)
}

pub static PROMPT_TO_CONVENTIONAL_COMMIT_PREFIX: &str =
    include_str!("../prompts/conventional_commit.tera");
pub static PROMPT_TO_SUMMARIZE_DIFF: &str = "You are an expert programmer summarizing a git diff.
Reminders about the git diff format:
For every file, there are a few metadata lines, like (for example):
```
diff --git a/lib/index.js b/lib/index.js
index aadf691..bfef603 100644
--- a/lib/index.js
+++ b/lib/index.js
```
This means that `lib/index.js` was modified in this commit. Note that this is only an example.
Then there is a specifier of the lines that were modified.
A line starting with `+` means it was added.
A line that starting with `-` means that line was deleted.
A line that starts with neither `+` nor `-` is code given for context and better understanding.
It is not part of the diff.
After the git diff of the first file, there will be an empty line, and then the git diff of the next file.

Do not include the file name as another part of the comment.
Do not use the characters `[` or `]` in the summary.
Write every summary comment in a new line.
Comments should be in a bullet point list, each line starting with a `-`.
The summary should not include comments copied from the code.
The output should be easily readable. When in doubt, write fewer comments and not more. Do not output comments that
simply repeat the contents of the file.
Readability is top priority. Write only the most important comments about the diff.

EXAMPLE SUMMARY COMMENTS:
```
- Raise the amount of returned recordings from `10` to `100`
- Fix a typo in the github action name
- Move the `octokit` initialization to a separate file
- Add an OpenAI API for completions
- Lower numeric tolerance for test files
- Add 2 tests for the inclusive string split function
```
Most commits will have less comments than this examples list.
The last comment does not include the file names,
because there were more than two relevant files in the hypothetical commit.
Do not include parts of the example in your summary.
It is given only as an example of appropriate comments.

CONSIDER THE FOLLOWING COMMIT MESSAGE FOR CONTEXT:

```
{{ commit_message }}
```

THE GIT DIFF TO BE SUMMARIZED:
```
{{ file_diff }}
```

THE SUMMARY: ";
pub static PROMPT_TO_SUMMARIZE_DIFF_SUMMARIES: &str = "You are an expert programmer writing a commit message.
You went over every file that was changed in it.
For some of these files changes where too big and were omitted in the files diff summary.
Please summarize the commit.
Write your response in bullet points, using the imperative tense.
Starting each bullet point with a `-`.
Write a high level description. Do not repeat the commit summaries or the file summaries.
Write the most important bullet points. The list should not be more than a few bullet points.

{% if commit_message %}
CONSIDER THE FOLLOWING COMMIT MESSAGE FOR CONTEXT:

```
{{ commit_message }}
```
{% endif %}

THE FILE SUMMARIES:
```
{{ summary_points }}
```

Remember to write only the most important points and do not write more than a few bullet points.

THE COMMIT MESSAGE:";
pub static PROMPT_TO_SUMMARIZE_DIFF_TITLE: &str = "You are an expert programmer writing a commit message title.
You went over every file that was changed in it.
Some of these files changes were too big, and were omitted in the summaries below.
Please summarize the commit into a single specific and cohesive theme.
Write your response using the imperative tense following the kernel git commit style guide.
Write a high level title.
Do not repeat the commit summaries or the file summaries.
Do not list individual changes in the title.

EXAMPLE SUMMARY COMMENTS:
```
Raise the amount of returned recordings
Switch to internal API for completions
Lower numeric tolerance for test files
Schedule all GitHub actions on all OSs
```

CONSIDER THE FOLLOWING COMMIT MESSAGE FOR CONTEXT:

```
{{ commit_message }}
```

THE FILE SUMMARIES:
```
{{ summary_points }}
```

Remember to write only one line, no more than 50 characters.
THE COMMIT MESSAGE TITLE:";
pub static PROMPT_TO_TRANSLATE: &str = include_str!("../prompts/translation.tera");
