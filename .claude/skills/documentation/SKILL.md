---
name: documentation
description: Create or update concise technical documentation that remains consistent with code, requirements, and architecture decisions.
argument-hint: "[document or changed behavior]"
---

Document `$ARGUMENTS` for the actual reader and task.

## Select the document type

Use one primary purpose per document:

- **Tutorial:** a safe learning path for a new user.
- **How-to guide:** steps for a competent user to accomplish a concrete goal.
- **Reference:** accurate facts, interfaces, configuration, commands, or schemas.
- **Explanation:** rationale, trade-offs, concepts, and design context.

## Writing rules

- Start with the reader's goal and required context.
- State prerequisites, supported scope, and important limitations.
- Prefer concrete commands and examples that are checked against the repository.
- Explain why only where it affects decisions or prevents misuse.
- Keep one source of truth; link rather than duplicate volatile facts.
- Use stable terminology from the code and domain.
- Make headings descriptive and paragraphs focused.
- Distinguish normative requirements from examples and suggestions.
- Include failure recovery and verification steps in operational guides.
- Update nearby indexes and links when adding or moving documentation.
- For a durable architectural choice, create or update an ADR instead of hiding rationale in a how-to guide.

Before finishing, test the document as a fresh reader: can the intended audience act correctly without unstated knowledge, and would a future maintainer know which claims must change when the code changes?
