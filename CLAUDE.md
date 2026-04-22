# pagerduty-cli

## PagerDuty Concepts

### Priority vs Urgency

**Urgency is NOT a knob to reach for. Priority is.**

- **Priority** (P1-P4) is the user-facing severity level. It is the concept responders think and talk about. All IM configuration, workflows, and response posture are keyed on priority.
- **Urgency** (high/low) is a PD internal field that controls whether PD phones/pages someone. It is a consequence of priority, not a parallel control.

Do not suggest urgency as a solution to paging behavior problems. The correct fix is always at the priority level - either by configuring services to derive urgency from priority, or by using the correct escalation policy for the incident's priority tier.
