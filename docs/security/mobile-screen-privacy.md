# Mobile screen privacy

Muxport treats source code, commands, approvals, account metadata, and future
credential-entry views as sensitive display content.

## Android

`MainActivity` applies `FLAG_SECURE` before Flutter renders. Android therefore
blocks screenshots, ordinary screen recording, and recent-app preview capture
for the entire activity. The protection is intentionally app-wide so a newly
added sensitive screen cannot accidentally omit it.

## iOS

iOS does not offer an application API that prevents a user-initiated screenshot.
Muxport covers every scene window with an opaque native view as the scene
resigns active and removes it only after the scene becomes active. This keeps
application content out of the app-switcher snapshot. Credential-entry views
must additionally clear their transient fields when they lose focus or the app
leaves the foreground.

These controls reduce accidental disclosure; they do not defend against a
compromised operating system, external camera, or a modified client build.
