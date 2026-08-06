# `declared_placement_cannot_grant` — challenge dimensions

The world an author *names* must be able to grant what the body needs.

| axis | file | why |
|---|---|---|
| direct invalid | `direct.pw` | R-025's shape: a device read under `placement origin` |
| indirect invalid | `caught.pw` | a different world and family, reached through a helper |
| neighbour | `browser-can-grant-device.pw` | the same device read where the device is |
| neighbour | `origin-can-grant-secret.pw` | the same secret read where secrets live |
| semantic preservation | `helper-chain.pw` | two helpers deep — moving code into a helper must not change the verdict |

Two neighbours because the rule is a *pairing* of world and capability: one
proves it does not ban the capability, the other that it does not ban the
world.

**Not relevant.** *Rebinding* and *branch join* are about values; this rule is
about effects, which have no join to get wrong.
