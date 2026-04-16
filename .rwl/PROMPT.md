# Ralph Wiggum Loop - ONE TASK THEN EXIT

You are in a Ralph Wiggum loop. You have NO MEMORY of previous runs.
Your state persists ONLY in `.rwl/progress.txt`.

## CRITICAL RULES

1. **READ .rwl/progress.txt FIRST** - It tells you what was done
2. **DO ONE SMALL THING** - Not a phase. One file, one fix, one test.
3. **EXIT IMMEDIATELY** - Do not retry errors. Just exit.

The loop will restart you with fresh context. That's the whole point.
Validation runs EXTERNALLY - you do NOT run tests or validation.

---

## Your Workflow

1. Read state: `cat .rwl/progress.txt && git log --oneline -10`
2. Do ONE small task
3. Record what you did in progress.txt
4. If ALL work is complete, signal: `{{completion_signal}}`
5. EXIT - do nothing else

---

## Implementation Plan

Read `{{plan_path}}` for what to build.
Each phase lists files and validation criteria.

{{#if progress}}
---

## Previous Iteration Feedback

The following feedback accumulated from previous iterations:

{{progress}}
{{/if}}

## Now: Read progress.txt and do ONE thing
