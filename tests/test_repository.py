"""Offline checks for repository metadata and documentation, using only stdlib."""

from collections import Counter
import hashlib
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tomllib
import unittest
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
# shortcut: checks inline links and ATX headings used here; extend for other Markdown forms.


def prose(markdown):
    """Ignore fenced examples when checking this repository's Markdown links."""
    fence = None
    lines = []
    for line in markdown.splitlines():
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})(.*)$", line)
        if fence:
            if marker and marker[1][0] == fence[0] and len(marker[1]) >= len(fence) and not marker[2].strip():
                fence = None
        elif marker:
            fence = marker[1]
        else:
            lines.append(line)
    if fence:
        raise ValueError(f"Unclosed code fence: {fence}")
    return "\n".join(lines)


def heading_anchors(markdown):
    counts = Counter()
    anchors = set()
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", prose(markdown), re.MULTILINE):
        slug = re.sub(r"[^\w\- ]", "", heading.lower()).replace(" ", "-")
        suffix = f"-{counts[slug]}" if counts[slug] else ""
        counts[slug] += 1
        anchors.add(slug + suffix)
    return anchors


class MarkdownCheckTests(unittest.TestCase):
    def test_unclosed_and_mismatched_code_fences_are_reported(self):
        for markdown in ["```rust\ncode", "~~~\ncode\n```", "````\n```\n"]:
            with self.subTest(markdown=markdown), self.assertRaisesRegex(ValueError, "Unclosed code fence"):
                prose(markdown)

    def test_fenced_examples_do_not_create_links_or_headings(self):
        self.assertEqual(prose(
            "before\n````md\n```\n# fake\n[x](missing)\n````\nafter\n"
            "~~~\n[also fake](missing)\n~~~\nend"
        ), "before\nafter\nend")

    def test_anchor_punctuation_unicode_and_duplicate_heading_suffixes(self):
        self.assertEqual(heading_anchors(
            "# Qt/QML (`offline`)\n## Привет, мир!\n## Same\n## Same\n## Same\n"
            "```md\n## invisible\n```\n"
        ), {"qtqml-offline", "привет-мир", "same", "same-1", "same-2"})


class RepositoryContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
            cwd=ROOT, capture_output=True, check=True,
        )
        cls.files = set(result.stdout.decode().rstrip("\0").split("\0"))
        cls.manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        cls.members = cls.manifest["workspace"]["members"]

    def test_local_documentation_links_and_heading_anchors(self):
        checked = 0
        for name in sorted(self.files):
            if not name.endswith(".md"):
                continue
            source = ROOT / name
            contents = prose(source.read_text(encoding="utf-8"))
            for target in re.findall(r"!?\[[^\]\n]+\]\(([^)\n]+)\)", contents):
                url = urlsplit(target.strip().strip("<>"))
                if url.scheme or url.netloc:
                    continue
                destination = source.parent / unquote(url.path) if url.path else source
                ignored = subprocess.run(
                    ["git", "check-ignore", "--no-index", "--quiet", "--", str(destination)],
                    cwd=ROOT, capture_output=True,
                )
                self.assertIn(ignored.returncode, [0, 1], ignored.stderr)
                if ignored.returncode == 0:
                    continue
                with self.subTest(source=name, target=target):
                    self.assertTrue(destination.exists(), f"Missing local target: {target}")
                    if url.fragment and destination.suffix == ".md":
                        self.assertIn(unquote(url.fragment), heading_anchors(destination.read_text(encoding="utf-8")))
                checked += 1
        self.assertGreater(checked, 0, "No local links were checked")

    def test_every_markdown_document_has_content_and_closed_examples(self):
        for name in sorted(self.files):
            if name.endswith(".md"):
                with self.subTest(file=name):
                    markdown = (ROOT / name).read_text(encoding="utf-8")
                    self.assertTrue(markdown.strip(), "Empty document")
                    prose(markdown)

    def test_player_fixture_readme_names_each_committed_tone(self):
        directory = ROOT / "crates/osu-radio-player/tests/fixtures"
        documented = set(re.findall(r"`([^`/]+\.(?:mp3|ogg|wav))`", (directory / "README.md").read_text(encoding="utf-8")))
        self.assertEqual(documented, {path.name for path in directory.iterdir() if path.suffix in {".mp3", ".ogg", ".wav"}})

    def test_bundled_attribution_notices_retain_their_full_contents(self):
        notices = {
            "fonts/OFL.txt": "580df76c95a1ec5ab878ceb25bb3d85c6a076804e9c970c8c6972aea775fdf65",
            "fonts/Poppins-OFL.txt": "6be04893d770899a015649c7aa3b582f871b272f8747a92b78b17c3e5c8b2573",
            "icons/LUCIDE-LICENSE": "b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57",
            "icons/REMIX-LICENSE": "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
        }
        assets = ROOT / "apps/osu-radio-qt/assets"
        sources = (assets / "SOURCES.md").read_text(encoding="utf-8")
        for name, expected in notices.items():
            with self.subTest(notice=name):
                self.assertIn(f"]({name})", sources)
                self.assertEqual(hashlib.sha256((assets / name).read_text(encoding="utf-8").encode("utf-8")).hexdigest(), expected)

    def test_workspace_and_path_dependencies_match_real_packages_and_lockfile(self):
        discovered = {path.parent.relative_to(ROOT).as_posix() for base in ["apps", "crates"]
                      for path in (ROOT / base).glob("*/Cargo.toml")}
        self.assertEqual(set(self.members), discovered)
        self.assertEqual(len(self.members), len(set(self.members)))
        locked = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))["package"]
        for member in self.members:
            manifest_path = ROOT / member / "Cargo.toml"
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            package = manifest["package"]
            with self.subTest(member=member):
                self.assertTrue(any(entry["name"] == package["name"]
                                    and entry["version"] == package["version"]
                                    and "source" not in entry for entry in locked))
                sections = [manifest.get(key, {}) for key in ["dependencies", "dev-dependencies", "build-dependencies"]]
                for target in manifest.get("target", {}).values():
                    sections.extend(target.get(key, {}) for key in ["dependencies", "dev-dependencies", "build-dependencies"])
                for section in sections:
                    for name, dependency in section.items():
                        if not isinstance(dependency, dict) or "path" not in dependency:
                            continue
                        dependency_file = (manifest_path.parent / dependency["path"] / "Cargo.toml").resolve()
                        self.assertTrue(dependency_file.is_file(), str(dependency_file))
                        actual = tomllib.loads(dependency_file.read_text(encoding="utf-8"))["package"]["name"]
                        self.assertEqual(actual, dependency.get("package", name))

    def test_manifest_entry_points_exist(self):
        for member in self.members:
            base = ROOT / member
            manifest = tomllib.loads((base / "Cargo.toml").read_text(encoding="utf-8"))
            with self.subTest(member=member):
                self.assertTrue((base / "src/lib.rs").is_file() or (base / "src/main.rs").is_file())
                build = manifest["package"].get("build")
                if isinstance(build, str):
                    self.assertTrue((base / build).is_file())
                targets = [manifest["lib"]] if "lib" in manifest else []
                for section in ["bin", "test", "bench", "example"]:
                    targets.extend(manifest.get(section, []))
                for target in targets:
                    if "path" in target:
                        self.assertTrue((base / target["path"]).is_file(), target["path"])

    def test_generated_files_and_local_docs_are_ignored_while_sources_and_specs_remain_visible(self):
        generated = ["target/test-output", "docs/review-report.md", "tools/fixture/obj/restore.json",
                     "docs/impl/queue-rework.md", "docs/agent-notes.md",
                     "docs/specs/notes.txt", "docs/specs/nested/spec.md", "docs/specs.md", "docs/notes.md",
                     "tools/fixture/bin/fixture", "apps/osu-radio-qt/tests/fixtures/__pycache__/server.cpython-314.pyc",
                     "osu-radio.db", "osu-radio.db-shm", "osu-radio.db-wal",
                     ".serena/cache/probe", ".serena/project.local.yml",
                     "apps/osu-radio-qt/.qmlls.ini", "apps/osu-radio-cli/target/probe",
                     "crates/radio-core/target/probe", "crates/radio-scanner/target/probe"]
        sources = ["docs/agent/index.md", "docs/agent/test-coverage.md", "docs/agent/nested/guide.md",
                   "docs/specs/queue-rework.md", "docs/specs/new-feature.md", "crates/radio-core/src/lib.rs",
                   "apps/osu-radio-qt/tests/test_tools.py", "tools/osu-lazer-realm-parser/Program.cs",
                   ".serena/project.yml", ".serena/nested/cache/probe", ".serena/nested/project.local.yml",
                   "apps/osu-radio-qt/nested/.qmlls.ini", "apps/osu-radio-cli/targeting/source.rs",
                   "crates/radio-core/targeting/source.rs", "crates/radio-scanner/targeting/source.rs"]
        result = subprocess.run(
            ["git", "check-ignore", "--no-index", "--stdin"], input="\n".join(generated + sources) + "\n",
            cwd=ROOT, text=True, capture_output=True,
        )
        self.assertIn(result.returncode, [0, 1], result.stderr)
        self.assertEqual(set(result.stdout.splitlines()), set(generated))

    def test_empty_legacy_metadata_has_no_submodules_or_lock_contents(self):
        self.assertEqual((ROOT / ".gitmodules").read_bytes(), b"")
        self.assertEqual((ROOT / "crates/radio-db/src/migrations/.diesel_lock").read_bytes(), b"")
        config = subprocess.run(["git", "config", "--file", ".gitmodules", "--list"],
                                cwd=ROOT, text=True, capture_output=True, check=True)
        self.assertEqual(config.stdout, "")
        index = subprocess.run(["git", "ls-files", "--stage", "-z"],
                               cwd=ROOT, text=True, capture_output=True, check=True)
        self.assertFalse(any(entry.startswith("160000 ") for entry in index.stdout.split("\0")))

    def test_rust_toolchain_declares_the_required_format_and_lint_components(self):
        toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]
        self.assertRegex(toolchain["channel"], r"^\d+\.\d+\.\d+$")
        self.assertEqual(toolchain["profile"], "minimal")
        self.assertEqual(set(toolchain["components"]), {"rustfmt", "clippy"})
        self.assertEqual(len(toolchain["components"]), 2)
        self.assertEqual(self.manifest["workspace"]["package"]["edition"], "2024")

    def test_agent_tooling_metadata_routes_to_the_existing_skill_and_local_workspace(self):
        # These are guards for the simple scalar/list forms committed here, not a YAML parser.
        skill = (ROOT / ".agents/skills/cr/SKILL.md").read_text(encoding="utf-8")
        name = re.search(r"^name: (\S+)$", skill, re.MULTILINE)[1]
        interface = (ROOT / ".agents/skills/cr/agents/openai.yaml").read_text(encoding="utf-8")
        self.assertTrue(interface.startswith("interface:\n"))
        fields = dict(re.findall(r'^  (\w+): "([^"\n]+)"$', interface, re.MULTILINE))
        self.assertEqual(set(fields), {"display_name", "short_description", "default_prompt"})
        self.assertEqual(fields["display_name"], name.upper())
        self.assertIn(f"${name}", fields["default_prompt"])
        self.assertTrue(all(value.strip() for value in fields.values()))
        serena = (ROOT / ".serena/project.yml").read_text(encoding="utf-8")
        for setting in ['project_name: "osu-radio"', 'encoding: "utf-8"',
                        "ignore_all_files_in_gitignore: true", "activation_command:",
                        "ls_workspace_folders:\n- \".\"", "ls_additional_workspace_folders: []",
                        "language_servers:\n- rust"]:
            with self.subTest(setting=setting):
                self.assertIn("\n" + setting + "\n", "\n" + serena)
        timeout = float(re.search(r"^activation_command_timeout: ([\d.]+)$", serena, re.MULTILINE)[1])
        self.assertGreater(timeout, 0)

    def test_linux_ci_command_matrix_uses_pinned_tools_and_scoped_backends(self):
        workflow = (ROOT / ".github/workflows/linux-ci.yml").read_text(encoding="utf-8")
        # Reject unsupported run forms rather than silently omitting a new command.
        runs = re.findall(r"^\s+(?:- )?run: (.+)$", workflow, re.MULTILINE)
        self.assertEqual(len(runs), len(re.findall(r"^\s+(?:- )?run:", workflow, re.MULTILINE)))
        self.assertTrue(runs)
        self.assertFalse(any(run.startswith(("|", ">")) for run in runs))
        self.assertEqual(runs.count("rustup toolchain install --no-self-update"), 3)
        self.assertEqual(set(re.findall(r"^  (\w+):\n    runs-on: ubuntu-24.04$", workflow, re.MULTILINE)),
                         {"formatting", "tests", "clippy"})
        cargo = []
        package_names = {tomllib.loads((ROOT / member / "Cargo.toml").read_text(encoding="utf-8"))["package"]["name"]
                         for member in self.members}
        for run in runs:
            with self.subTest(run=run):
                if shutil.which("bash"):
                    checked = subprocess.run(["bash", "-n"], input=run + "\n", text=True, capture_output=True)
                    self.assertEqual(checked.returncode, 0, checked.stderr)
                args = shlex.split(run)
                if args[0] != "cargo":
                    continue
                self.assertNotIn("--all-features", args)
                packages = {args[index + 1] for index, value in enumerate(args) if value == "-p"}
                self.assertLessEqual(packages, package_names)
                self.assertNotIn("osu-radio-qt", packages)
                if args[1] in {"test", "clippy"}:
                    self.assertIn("--locked", args)
                    if args[1] == "test":
                        self.assertIn("--test-threads=1", args)
                    else:
                        self.assertIn("--all-targets", args)
                        self.assertEqual(args[-3:], ["--", "-D", "warnings"])
                    features = args[args.index("--features") + 1] if "--features" in args else ""
                    self.assertIn(features, {"", "sqlite", "mock"})
                    self.assertEqual("--no-default-features" in args, features == "sqlite")
                    cargo.append((args[1], frozenset(packages), features, "--lib" in args))
        headless = frozenset(package_names - {"osu-radio-qt", "osu-radio-client"})
        server = frozenset({"osu-radio-server"})
        client = frozenset({"osu-radio-client"})
        self.assertCountEqual(cargo, [("test", headless, "", False), ("test", server, "sqlite", False),
                                     ("test", client, "", True), ("test", client, "mock", True),
                                     ("clippy", headless | client, "", False),
                                     ("clippy", server, "sqlite", False), ("clippy", client, "mock", False)])
        self.assertIn("cargo fmt --check", runs)
        for directory in ["tests", "crates/radio-scanner/tests", "tools/search-benchmark", "apps/osu-radio-qt/tests"]:
            self.assertTrue(any(run.startswith("python3 -B -m unittest discover -s " + directory + " ") for run in runs), directory)

    def test_coverage_worklist_lists_each_repository_file_once(self):
        worklist = (ROOT / "docs/agent/test-coverage.md").read_text(encoding="utf-8")
        entries = re.findall(r"^\| \[([^]]+)\]\([^\n]+?\) \| (?:Focused|Pending) \|", worklist, re.MULTILINE)
        self.assertEqual(len(entries), len(set(entries)), "Duplicate inventory entries")
        self.assertEqual(set(entries), self.files)

    def test_docs_only_contains_versioned_agent_guides_and_specification_markdown(self):
        for name in self.files:
            if name.startswith("docs/") and not name.startswith("docs/agent/"):
                with self.subTest(file=name):
                    self.assertEqual(Path(name).parent.as_posix(), "docs/specs")
                    self.assertEqual(Path(name).suffix, ".md")


if __name__ == "__main__":
    unittest.main()
