/**
 * Entry for dist/boot-guard.js, injected ahead of Conductor's entry chunk.
 *
 * Separate from the panel because it has to run before Conductor's own modules
 * take their reference to `fetch`. Injected into the panel's chunk it installs
 * too late and never sees the request.
 */

import { installMinimumVersionGuard } from "./compat.js";

installMinimumVersionGuard();
