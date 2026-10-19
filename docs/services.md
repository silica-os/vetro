# Services: interface coverage

`libvetro::services` (`crates/libvetro/src/services/`). Proxies are `#[zbus::proxy]` traits read
from the server implementations in silica-desktop `crates/daemon/src`; tests run them against
mock servers with the same signatures (`services/mock.rs`, private `dbus-daemon` in `/dev/shm`).
Bus name `dev.frascella.Silica` (daemon), object paths `/dev/frascella/Silica/<Name>`.

Notes: `State` of System1 is a *method*, not a property; `Changed` carries the whole dictionary.
Structure replies are plain types in `services::types`. Where the interface carries JSON
strings (System1 `Outputs`/`ConfigureOutput`/`ReportCompositor`, Clipboard1 `History`, Actions1
`List`) the proxy returns the raw string; parse defensively (no typed parser yet).

| Interface | Proxy | Methods | Properties | Signals |
| --- | --- | --- | --- | --- |
| Settings1 | `Settings1Proxy` | Get, GetAll, Set, SetMany, Reset | - | Changed(s, v) |
| System1 | `System1Proxy` | State, SetWifiEnabled, SetBluetoothPowered, SetVolume, SetMuted, SetBrightness, SetKeyboardBrightness, WifiNetworks, ConnectWifi, BluetoothDevices, ConnectBluetooth, AudioOutputs, SetAudioOutput, SetDoNotDisturb, SetDark, SetAppearanceMode, SetLook, SetTextReplacements, Outputs, ConfigureOutput, SetNightLight, SetFocus, Suspend, Unlock, ReportCompositor | - | Changed(a{sv}) |
| Apps1 | `Apps1Proxy` | List, Get, AppsFor, DefaultFor, SetDefault, Open, OpenUris, NoteLaunched, Uninstall | - | Added, Removed, Changed, DefaultChanged |
| Trust1 | `Trust1Proxy` | Assess, Prompt, SuggestMove, SetSandbox, RegisterAgent, Answer, Pending, Blocked, OpenAnyway | - | PromptRequested, PromptClosed |
| Volumes1 | `Volumes1Proxy` | Inspect, Attach, Detach, List | - | Attached, Detached |
| Updates1 | `Updates1Proxy` | Check, Update, UpdateAll | - | Progress, Finished |
| Thumbnails1 | `Thumbnails1Proxy` | Thumbnail, Icon, AppIcon, Invalidate | - | Ready |
| Share1 | `Share1Proxy` | Targets, Share | - | - |
| Notifications1 | `Notifications1Proxy` | List, Dismiss, Clear, InvokeAction, Reply | - | Added, Removed |
| org.freedesktop.Notifications | `FdoNotificationsProxy` | Notify, CloseNotification, GetCapabilities, GetServerInformation | - | NotificationClosed, ActionInvoked, NotificationReplied (Silica) |
| Clipboard1 | `Clipboard1Proxy` | History (JSON), Copy, CopyImage, Remove, Clear | - | Changed |
| Actions1 | `Actions1Proxy` | List (JSON) | - | - |
| Sound1 | `Sound1Proxy` | PlaySound, PlayAlert, AlertSounds, PlayVolumeFeedback, UiSoundsEnabled, SetUiSoundsEnabled | - | UiSoundsChanged |
| Authentication1 | `Authentication1Proxy` | Pending, Respond, Cancel | Registered | Requested, Closed |
| portal.FileChooser | `PortalFileChooserProxy` | OpenFile, SaveFile, SaveFiles | version | - |
| portal.OpenURI | `PortalOpenUriProxy` | OpenURI, OpenFile, OpenDirectory | version | - |
| portal.Settings | `PortalSettingsProxy` | ReadAll, Read | version | SettingChanged |
| portal.Request | `PortalRequestProxy` | Close | - | Response |

## Watchers (all `async fn run(&self, &Connection)`, no spawning, never fail)

| Value | Source | Observable |
| --- | --- | --- |
| `Appearance` (dark, increase contrast, reduce transparency, reduce motion, accent, highlight, full keyboard access, text scale) and `SystemLook` (adds sidebar size, scroll bars, scroll click, icon style, folder colour, glass, window tint, double-click time) | System1 `State` + `Changed` (`appearance.dark`, `look.*`) | `Services::appearance()`, `look()` |
| All settings keys | Settings1 `GetAll` + `Changed` | `Services::settings()` |
| `KeyRepeat { delay_ms, rate_hz }` | `keyboard.repeatDelay` (100-3000, 375), `keyboard.repeatRate` (0-100, 11) | `Services::key_repeat()` |
| `Option<RendererBackend>` | `renderer.backend` (proposed, see gaps) | `Services::renderer()` |

Lifecycle: the watchers subscribe first, then read; they re-read when `dev.frascella.Silica`
gets an owner (`NameOwnerChanged`) and fall back to defaults when it loses it. A bus that has
no daemon, or no bus at all, leaves the defaults (`Services::run` returns `Error::NoBus` only
for the missing bus).

## Gaps and discrepancies

- **No appearance keys in Settings1.** Appearance, accent, highlight and the accessibility
  switches live only in System1's `State` (`look.*`); the schema of Settings1 has none of them.
  The appearance watcher therefore reads System1 only.
- **No renderer-backend key.** Settings1's schema has no place for the per-machine renderer
  choice; a `Set` of an unknown key is refused. Vetro reads `renderer.backend`
  (`"vulkan" | "gl" | "software"`) if the daemon adds it (`RENDERER_KEY`); until then
  `Services::renderer()` is `None`. Needs a key in `crates/daemon/src/settings.rs`.
- **`SetLook` takes only some keys.** The doc comment of `SetLook` lists `accent`, `highlight`,
  `sidebarIconSize`, `scrollBars`, `scrollClick`, `iconStyle`, `folderColour`, `glass`,
  `windowTint`, but `State` also reports `look.keyboardNavigation`, `doubleClickMs`,
  `reduceTransparency`, `reduceMotion`, `increaseContrast`, `textScale` (set elsewhere in the
  daemon). The proxy's `set_look` passes any dictionary through.
- **Highlight colour.** The daemon sends a name (`look.highlight`), no tone of its own; Vetro
  uses the accent colour of that name. `automatic` is `None`.
- **System1.SetDoNotDisturb** has no reply value on the server (fire and forget).
- **State is a method.** System1 has no properties; every client reads `State()` and `Changed`.
- **`Trust1.RegisterAgent`** identifies the agent by the caller's bus name; the agent-side
  interface the daemon calls back (prompting) is not implemented in Vetro yet. Same for the
  polkit `AuthenticationAgent` (daemon side only).
- **JSON replies** (listed above) are raw strings, no typed parser.
- **Portals:** proxies only. No request helper (`Request` path from `sender`/`handle_token`).
- Settings1 `Announce` is a Rust-side helper of the server, not a bus method.
