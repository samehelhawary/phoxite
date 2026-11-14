# Security

## Reporting a vulnerability

Report it privately through GitHub: on the repository's **Security** tab choose
**Report a vulnerability**. Please don't open a public issue for it.

Include what is affected (the language server, the VS Code extension, or both), the version,
and the steps or a project that shows the problem.

## What counts

Phoxite reads the code of the project you open. These are in scope:

- Opening or indexing a project runs code or commands the user didn't ask for. The server
  parses PHP and never executes it; the extension starts a process only for what the user
  runs (an Artisan command).
- A file of a project makes the server read or write outside the project and its cache
  folder.
- A crafted file crashes the server or makes it hang.

## Supported versions

Fixes go into the latest release.
