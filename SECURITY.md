# Security policy

Please report vulnerabilities privately through GitHub's private vulnerability
reporting: **Security → Report a vulnerability** on
https://github.com/Modusmundi/rustyrian. Do not open public issues for
security problems.

Include the affected version or commit, the input that triggers the problem
(a netplay packet sequence, a data/save/config/demo file), and what happens.

The untrusted inputs are netplay UDP packets (remote) and data, save, config
and demo files (local or shared between users). Reports are acknowledged within
7 days. Fixes ship in a patch release; the reporter is credited unless they ask
otherwise.

Vulnerabilities that also affect upstream OpenTyrian are reported to the
upstream maintainers privately before any public disclosure.
