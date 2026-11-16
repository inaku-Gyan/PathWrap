# Native navigation of another application's file dialog

Research date: 2026-11-17. Resolves [Investigate native navigation of another application's file dialog](https://github.com/inaku-Gyan/PathWrap/issues/62), under [Native folder navigation and Ctrl+G focus](https://github.com/inaku-Gyan/PathWrap/issues/61).

This is a source-based feasibility report. No application was launched, no interface was probed at runtime, and no implementation or GUI test was run. “Documented” below means a Microsoft API contract; Wine compatibility tests are identified separately and do not establish a Windows support guarantee.

## Finding

Native folder navigation is supported **after obtaining the correct existing dialog or browser interface**. This investigation found no documented, general-purpose API that turns the HWND of an arbitrary already-open, other-process Open/Save dialog into its `IFileDialog` or `IShellBrowser`. That is an evidence limit, not proof that external discovery is impossible.

The strongest external-only discovery candidate is a provider-exposed object model: UI Automation's ObjectModel pattern explicitly marshals an object to the client, while MSAA's native-object protocol can expose an interface when the provider implements it. Neither contract promises that Windows file-dialog providers expose the required navigation interface. A bounded discovery prototype can answer that question without simulated input.

A component already inside the host's Shell site chain has a documented route to `IShellBrowser`. An in-process helper also has an undocumented legacy HWND-to-browser lead. These are materially different integration boundaries; they remain candidates for human evaluation rather than approved product architecture.

## Route comparison

| Route | Discovery evidence and boundary | Navigation and completion | Feasibility status |
| --- | --- | --- | --- |
| Owner cooperates and retains its `IFileDialog` | Documented owner-created object; PathWarp would need an application-specific IPC bridge or plugin | `SetFolder`; folder events and `GetFolder` | Strong documented control route for cooperating modern hosts; does not attach to arbitrary hosts |
| UIA ObjectModel pattern | Documented cross-process object marshaling; provider must expose the pattern and useful native object | Query returned object for supported native interfaces, then use their navigation/readback methods | Best external-only native-discovery prototype; file-dialog exposure unverified |
| MSAA `AccessibleObjectFromWindow`, native object, service queries | Documented accessibility/native-object protocol, conditional on provider support | An actual exposed `IFileDialog` or browser/service object would be needed | Candidate probe; accessibility access alone is insufficient |
| Shell site chain `IServiceProvider::QueryService` | Documented when a component already has access to the browser's site chain | `IShellBrowser::BrowseObject`; current-view folder readback | Plausible helper/extension route; obtaining a site for every target dialog is unproven |
| Legacy `WM_USER + 7` browser retrieval | Undocumented message; primary Wine test uses it inside an OFN dialog hook | Returned local `IShellBrowser`, then `BrowseObject` and view readback | Concrete helper prototype lead for Explorer-style legacy dialogs; modern coverage unknown |
| `IShellWindows` enumeration | Documented access to registered Shell windows, not arbitrary HWNDs | Registered Explorer object's browser/service chain | Useful Explorer control route; no common-dialog coverage guarantee found |
| Existing `IExplorerBrowser` supplied by a host | Documented navigable browser object, but no general external HWND attachment contract found | `BrowseToIDList`/`BrowseToObject`; navigation-complete/failed events | Conditional alternative if the actual embedded instance is exposed or captured |
| Legacy `CDM_*` messages | Documented Explorer-style common-dialog control/readback messages | Folder getter exists; listed messages provide no folder-navigation setter | Observation aid for a helper, not a complete native navigation route |

The evidence and qualifications for each row follow.

## Modern Common Item Dialog: control is documented, attachment is not

Microsoft's [Common Item Dialog documentation](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog) describes creating `CLSID_FileOpenDialog` or `CLSID_FileSaveDialog` in the calling process and retaining their interfaces. Creating another instance and passing a target HWND to `Show` creates/shows that instance with an owner; it does not recover the existing dialog object. The documentation also demonstrates the **interface-to-HWND** direction through `IOleWindow::GetWindow`, not an inverse HWND lookup.

[`IFileDialog::SetFolder`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-setfolder) explicitly navigates when called while the dialog is displayed. Thus “only an initial-folder setter” is incorrect. Its `IShellItem` argument represents the destination. Its success HRESULT does not, by itself, establish PathWarp's required end state.

For a cooperating host, a small native bridge can retain the existing object, execute the request on the dialog's UI thread, and return observed state over IPC. This is a proposed architecture using documented owner APIs, not a standard bridge supplied by Windows. A helper loaded after the dialog has opened still needs to discover its interface; loading a DLL alone does not solve discovery. Capturing creation before `Show` would require earlier installation and cannot retrospectively capture a call that already happened.

## External accessibility and object-model discovery

[`AccessibleObjectFromWindow`](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-accessibleobjectfromwindow) requests an object/interface associated with an HWND. It permits supported interface IIDs and can return `E_NOINTERFACE`. This wording is not a guarantee that arbitrary interfaces belonging to the surrounding application are retrievable.

Microsoft's [native-object-model protocol](https://learn.microsoft.com/en-us/windows/win32/winauto/exposing-a-native-object-model-interface) requires the UI element to respond to `WM_GETOBJECT/OBJID_NATIVEOM` using `LresultFromObject`. [MSAA retrieval](https://learn.microsoft.com/en-us/windows/win32/winauto/retrieving-an-iaccessible-object) distinguishes a server-provided native accessible object from an Oleacc proxy. Receiving a generic accessibility proxy does not prove access to the dialog's native COM object or site.

Similarly, [Microsoft's `IAccessibleEx` example](https://learn.microsoft.com/en-us/windows/win32/winauto/using-iaccessibleex-from-a-client) queries `IServiceProvider` and then the specific `IAccessibleEx` service, with an explicitly in-process example assumption. It is not a contract that an external `IAccessible` exposes `SID_STopLevelBrowser`, `SID_SCommDlgBrowser`, or `IFileDialog`. The actual object, services, and cross-process proxy support need separate evidence.

UIA provides a more explicit possible transport: [ObjectModel control pattern](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingobjectmodel) says UIA marshals the provider's object pointer to the client process, which can use `QueryInterface` for the desired IID. [`GetUnderlyingObjectModel`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationobjectmodelpattern-getunderlyingobjectmodel) returns `IUnknown`. The pattern has no associated events; it is an entry point, not a navigation API or completion signal.

The [documented discovery sequence](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-understandingtheuiautomationtextobjectmodel) checks `UIA_IsObjectModelPatternAvailablePropertyId`, obtains `UIA_ObjectModelPatternId`, and retrieves the underlying model. The example concerns controls exposing a DOM. No source examined promises this pattern, or useful native dialog/browser IIDs, on standard file-dialog elements. Probe both the target root and relevant view children; do not assume either provider availability or that an address-bar text model is the dialog's navigation object.

## Shell browser and site-chain routes

The [`IShellBrowser` contract](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishellbrowser) explicitly names the Open File common dialog as an implementation. Objects that **already have access to the browser's site chain** can request `IShellBrowser` through `IServiceProvider::QueryService`, using services such as `SID_STopLevelBrowser` and `SID_SCommDlgBrowser`. This is a real documented native route for a suitably sited component, not a method to manufacture a site from an HWND.

[`BrowseObject`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellbrowser-browseobject) accepts a PIDL. `SBSP_ABSOLUTE | SBSP_SAMEBROWSER` expresses an absolute destination and navigation in the same browser, avoiding default settings that can create a new Explorer window. Acceptance by particular dialog hosts, host state synchronization, and preservation of pending filename/type still need runtime evidence.

A Shell extension or host plugin that actually receives the needed site could forward PathWarp requests to this browser. Its activation scope, availability across folders and hosts, lifetime, and how PathWarp identifies the correct live dialog all remain work. This is not evidence that installing an arbitrary extension yields a persistent browser reference in every Open/Save dialog.

[`IExplorerBrowser`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iexplorerbrowser) is another documented browser with navigation, current-view, and event methods. Creating `CLSID_ExplorerBrowser` yields a separate browser; it does not attach to the target. An existing instance could be useful only if obtained from the actual host. Its documented `SID_SExplorerBrowserFrame` discussion concerns host callbacks such as `ICommDlgBrowser`; it does not promise an external `IFileDialog` discovery service.

## Why ShellWindows does not close the discovery gap

[`IShellWindows`](https://learn.microsoft.com/en-us/windows/win32/api/exdisp/nn-exdisp-ishellwindows) is a collection of windows registered with `Register` or `RegisterPending`. [`FindWindowSW`](https://learn.microsoft.com/en-us/windows/win32/api/exdisp/nf-exdisp-ishellwindows-findwindowsw) searches that collection by location/cookie and returns a dispatch object; it is not an arbitrary HWND-to-COM operation. The [documented window categories](https://learn.microsoft.com/en-us/windows/win32/api/exdisp/ne-exdisp-shellwindowtypeconstants) list Explorer, browsers, callbacks, and desktop, with no common-dialog category.

Microsoft's [Explorer HWND example](https://devblogs.microsoft.com/oldnewthing/20040720-00/?p=38393) enumerates this collection, matches `IWebBrowserApp::HWND`, and requests `SID_STopLevelBrowser`. That establishes a useful out-of-process route for registered Explorer windows. No reviewed contract states that arbitrary application file dialogs register themselves there. Treat coverage as unestablished rather than asserting either universal inclusion or universal exclusion.

## Legacy dialogs and the undocumented helper lead

Keep three families separate: modern Vista-and-later Common Item Dialogs, legacy Explorer-style `GetOpenFileName/GetSaveFileName` dialogs, and Windows 3.1-style legacy dialogs. Microsoft's [legacy dialog documentation](https://learn.microsoft.com/en-us/windows/win32/dlgbox/open-and-save-as-dialog-boxes) distinguishes Explorer-style hook/control behavior from old-style customization. API family and host customization matter more than a familiar visual appearance.

A concrete primary-source lead exists in [Wine's pinned common-dialog compatibility test](https://github.com/wine-mirror/wine/blob/db11d0fe6a169c457e23d007e20404643d067aa8/dlls/comdlg32/tests/filedlg.c): `create_view_window2_hook` handles `CDN_FOLDERCHANGE`, sends `WM_USER + 7` to the parent dialog, casts the result to `IShellBrowser*`, and queries its active Shell view. This is an **in-process Explorer-style legacy hook example**, not an external-control demonstration or Microsoft support contract. It does not establish modern dialog support.

Microsoft's [`SendMessage` contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagew) supplies system marshaling only below `WM_USER`; higher messages require custom marshaling across processes. Therefore a foreign-process numeric result from this private message must not be treated as a callable local COM pointer. [`CoMarshalInterface`](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-comarshalinterface) describes the separate work required to produce client proxy data. A helper could instead keep browser calls local and send ordinary request/result data through its own IPC. Neither approach establishes missing interface-discovery support.

For legacy observation, [`CDN_FOLDERCHANGE`](https://learn.microsoft.com/en-us/windows/win32/dlgbox/cdn-folderchange) notifies an OFN hook when a folder opens, and [`CDM_GETFOLDERPATH`](https://learn.microsoft.com/en-us/windows/win32/dlgbox/cdm-getfolderpath) reads that folder for `OFN_EXPLORER` dialogs. The latter passes a buffer pointer in a message above `WM_USER`; it is not a direct external read into a local buffer. [`CDM_SETCONTROLTEXT`](https://learn.microsoft.com/en-us/windows/win32/dlgbox/cdm-setcontroltext) only sets control text. Combining filename edits with fabricated OK commands would be a different input/control-driving route and does not establish a native folder-navigation setter.

## Helper deployment, threading, and privilege boundaries

[`SetWindowsHookEx`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw) documents DLL-based hooks into another process/thread. This makes “native helper inside the dialog host” a technically meaningful option, independent of simulated typing. It also entails executing PathWarp code in another application's process. A hook merely provides an execution opportunity; native object acquisition, supported APIs, lifetime, and IPC still need design and evidence.

An injected DLL must match the target process architecture; [WOW64 documentation](https://learn.microsoft.com/en-us/windows/win32/winprog64/wow64-implementation-details) also requires CPU architecture matching, not just pointer width. Hook documentation describes additional Store-app/broker limitations. Do not extrapolate one x64 desktop-host result to x86, ARM, brokered pickers, or custom dialogs.

[UIPI documentation](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-allow-uiaccess-applications-to-prompt-for-elevation-without-using-the-secure-desktop) states that lower-integrity processes are restricted from sending messages or installing hooks into higher-integrity processes. Equal-integrity success would not prove elevated-host support. COM/IPC access control must also be evaluated for the selected transport rather than assuming window-message permission grants COM access.

For accessibility discovery, [UIAccess guidance](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview) adds trust, signing, installation, privilege, and desktop constraints. UIAccess is not an unconditional elevation mechanism and is intended for assistive technology scenarios. This report does not select UIAccess or elevation as PathWarp's policy.

[COM STA rules](https://learn.microsoft.com/en-us/windows/win32/com/single-threaded-apartments) require interface marshaling across apartments and message pumping for calls/callbacks. Prefer executing local helper navigation on the owning dialog's UI thread; a newly created remote thread is not automatically that apartment. Any prototype must expose timeouts, reentrancy, closed-dialog races, and release/unhook behavior without blocking the host's message loop.

## Evidence for a completed folder jump

A proposed observation sequence is:

1. Bind the request to the live target dialog and its process/thread; correlate any returned native object with that target rather than another Explorer window or new dialog.
2. Record the requested destination, current folder, pending filename, and file-type selection.
3. Issue the native request and record its HRESULT separately from observed completion.
4. Read the actual folder after navigation and compare its Shell identity with the destination; require the same dialog to remain open.
5. Return focus to that dialog after the verified jump, then observe focus separately.

For `IFileDialog`, [`GetFolder`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfolder) reads the current folder while displayed. [`OnFolderChange`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialogevents-onfolderchange) is also raised at initial opening, so the event alone is not request-specific proof. [`OnFolderChanging`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialogevents-onfolderchanging) allows the host to veto or redirect navigation; actual navigation is deferred until the callback returns.

For a browser route, query its active view, use [`IFolderView::GetFolder`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifolderview-getfolder), and, where supported, [`IPersistFolder2::GetCurFolder`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ipersistfolder2-getcurfolder). Reacquire the current view after navigation rather than trusting a stale view. An available `IExplorerBrowser` has [navigation-complete/failed events](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iexplorerbrowserevents-onnavigationcomplete). These observations still require an acquired interface; they are not discovery mechanisms.

[`IShellItem::Compare`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitem-compare) provides an identity comparison with explicit same/different/error results. A filesystem-path display string or a changed address-bar label alone is weaker evidence. Exact handling of aliases, libraries, virtual folders, redirects, and network failures needs a scoped acceptance definition.

[`GetFileName`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfilename) reads pending edit-box text, and [`GetFileTypeIndex`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-getfiletypeindex) reads the selected type while open. Test preservation explicitly; no reviewed `SetFolder` contract establishes it for every host or browser alternative.

Folder navigation and focus transfer are separate outcomes. [`SetForegroundWindow`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) is subject to foreground restrictions and can fail even when documented eligibility conditions hold. A successful native folder call must not be reported as proof of dialog focus. Ctrl+G policy remains a separate human decision.

## Candidate-specific prototypes and unresolved decisions

1. **External-only native-interface discovery.** On representative already-open modern Open/Save and Explorer-style legacy dialogs, inventory UIA ObjectModel availability, underlying-object IIDs, MSAA native-object responses, and supported service queries. Record HWND/process correlation, HRESULTs, proxy behavior, and whether useful folder readback succeeds. A positive result must demonstrate the correct existing dialog. Negative results establish only the tested host/build coverage.
2. **Native helper/browser route, if the integration boundary is accepted.** On the target UI thread, test the private legacy message and actual supplied Shell-site services independently. Test modern dialogs separately; do not assume legacy retrieval survives. Demonstrate `BrowseObject` navigation, current-folder readback, host filename/type synchronization, closed-dialog races, and helper unloading. The capability of loading a hook DLL is not sufficient success evidence.
3. **Cooperating modern-host reference.** Retain a real already-displayed `IFileDialog` in a controlled host, accept IPC, and measure `SetFolder`, events/readback, veto/redirect behavior, filename/type preservation, and focus return. This establishes navigation semantics and observation quality; it cannot prove arbitrary-host interface discovery.

The human decisions are the permitted native integration boundary (external provider APIs, host cooperation, and/or in-process code), compatibility coverage, and the acceptance rule for folder identity and preserved dialog state. No backend is selected here. Native navigation remains the preferred direction; if external native discovery is absent in the chosen coverage and helpers are declined, the separately scoped [address-bar UI Automation investigation](https://github.com/inaku-Gyan/PathWrap/issues/63) is the next candidate.
