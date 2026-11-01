# Windows installer candidate

`build.py` builds and stages a candidate from a Windows x64 MSVC developer
environment. It needs Python 3.11+, the repository Rust toolchain and Windows
target, .NET 10 SDK, matching MSVC Qt including QML tools, Inno Setup, and the
MSVC app-local redistributable DLLs. Run from the repository checkout:

```powershell
python packaging/windows/build.py --qt C:\Qt\6.8.3\msvc2022_64 --redist C:\path\to\x64\Microsoft.VC143.CRT --iscc "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" --materials C:\path\to\license-files --work C:\path\to\new-build-directory --qt-version 6.8.3 --dotnet-sdk 10.0.113 --msvc-version $env:VCToolsVersion --windows-sdk $env:WindowsSDKVersion --iscc-sha256 <ISCC_SHA256>
```

Paths and versions above are examples, not validated pins. `--materials` is a
nonempty directory of license and notice files to include. The CI wrapper uses
the existing project and Arch notice files; their presence does not establish a
complete Windows dependency inventory. It builds the self-contained
helper and GUI/server, invokes `windeployqt`, copies supplied runtime DLLs into
both executable directories, checks selected required files, and builds Setup.
It checks the supplied exact tool versions and ISCC hash before building, and
records a source/tool summary, explicit clean/dirty status, payload hashes and
`setup.sha256`. The [release workflow](../release/README.md) uploads these with
the licenses as the Windows artifact. It does not independently
pin or verify the origin of Qt, the SDKs or runtime DLLs, nor validate all
imports. Use an immutable clean source and verify Qt
source correspondence, all runtime dependencies, and actual install/update/
uninstall before release. A dirty checkout is only a local candidate.

Run `python -m unittest discover -s packaging/windows -p 'test_*.py' -v` for the
missing-file and tool-pin checks. A diagnostic candidate was built in GitHub
Actions and partially exercised with Wine; its Windows license materials are
not audited. See [T08](../../docs/impl/first-release-packaging/T08.md) for the
exact run and remaining checks.
