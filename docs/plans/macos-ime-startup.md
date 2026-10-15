# macOS IME startup ordering

Initialize GPUI's registered `GPUIApplication` singleton on the main thread
before `Application::new()`. GPUI 0.2.2 otherwise queries TIS keyboard state before
creating NSApplication, which can prevent marked-text composition in a
nonactivating panel. See the upstream reproduction in
[zed#62661](https://github.com/zed-industries/zed/issues/62661).

Do not create the base NSApplication singleton: GPUI needs its own subclass and
private ivars. Fail explicitly if that class is unavailable. This adapter is
macOS-only and must precede GPUI construction; no provider, storage, window
activation or user input-source setting changes are required.

Keep the existing native responder restoration after removing the titled style.
That fixes a separate ordinary-keyboard failure and does not replace correct IME
initialization. All task, tag, goal and search fields share the same native view.

Acceptance must use a real input method: select system Simplified Pinyin, type a
known phrase, observe composition/candidates and confirm the Chinese result with
Space. Check candidate navigation, cancellation and Return without starting a
task prematurely. Repeat after dialog dismissal and process restart. ASCII
injection or Unicode clipboard text cannot establish IME acceptance.
