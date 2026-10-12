"""Capture genuine CLI output. Linux + bubblewrap; no host managed configs or network.
Reads only synthetic config. The CLI binary must already exist; never builds Rust.
Path normalization is explicitly documented in the generated capture metadata.
"""
import json, os, pathlib, subprocess, tempfile, shutil, hashlib
ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = pathlib.Path(os.environ.get("CONFIGLENS_BIN", ROOT.parent / "target/debug/configlens")).resolve()
assert BIN.is_file(), "Build the Rust binary separately before capturing."
assert shutil.which("bwrap"), "bubblewrap is required; do not run without managed-path isolation"
fixtures = json.loads((ROOT / "src/data/fixtures.json").read_text())
captures = {}; checks = []
def write(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, indent=2) + "\n")
with tempfile.TemporaryDirectory(prefix="configlens-site-", dir=os.environ.get("TMPDIR")) as temp:
    sandbox = pathlib.Path(temp)
    for fixture in fixtures:
        root = sandbox / fixture["id"]; home = root / "home"; project = root / "project"
        cwd = project if fixture["client"] == "claude" else project / "packages/app"
        cwd.mkdir(parents=True); home.mkdir(); (project / ".git").mkdir()
        if fixture["client"] == "claude":
            paths = [home / ".claude/settings.json",project / ".claude/settings.json",project / ".claude/settings.local.json"]
        else:
            paths = [home / ".config/opencode/opencode.json",project / "opencode.json",cwd / "opencode.json"]
        for path,value,included in zip(paths,fixture["values"],fixture["included"]):
            write(path, {"model":value} if included else {})
        # Protected key demonstration is derived from the CLI's projection, not a JS mask.
        if fixture["id"] == "claude-local":
            write(paths[1], {"model":fixture["values"][1],"env":{"APP_TOKEN":"synthetic-not-a-credential"}})
        context = root / "context.json"
        write(context, {"schema_version":"1.0","client":fixture["client"],"client_version":fixture["version"],"project_trust":"trusted","environment":{},"sources":[]})
        env = {"HOME":str(home),"XDG_CONFIG_HOME":str(home / ".config"),"NO_COLOR":"1"}
        common = ["--client",fixture["client"],"--client-version",fixture["version"],"--cwd",str(cwd),"--project-root",str(project),"--input-context",str(context),"--allow-read-root",str(root),"--color","never"]
        def run(args):
            command = ["bwrap","--ro-bind","/","/","--bind",str(root),str(root),"--tmpfs","/etc","--unshare-net","--chdir",str(cwd),"--",str(BIN),*args]
            return subprocess.run(command,env=env,text=True,capture_output=True,check=False)
        def normalize(text): return text.replace(str(root),"$SANDBOX")
        def capture(name,args):
            result = run(args)
            assert result.returncode == 0, (args,result.stderr,result.stdout)
            captures[name] = {"command":"configlens " + ' '.join(normalize(str(a)) for a in args),"stdout":normalize(result.stdout),"stderr":normalize(result.stderr),"exitCode":result.returncode}
            return result
        result = run(["explain","/model",*common,"--format","json"])
        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        resolution = next(r for r in report["resolutions"] if r["pointer"] == "/model")
        expected = [v for v,i in zip(fixture["values"],fixture["included"]) if i][-1]
        assert resolution["observed_value"]["value"] == expected, resolution
        assert resolution["status"] == "partial" and "effective_value" not in resolution
        assert resolution["support"] == "fixture_verified"
        checks.append({"fixture":fixture["id"],"observed":expected,"status":"partial","support":"fixture_verified","effectiveWithheld":True})
        if fixture["id"] == "claude-local":
            capture("clients",["clients"])
            capture("scan",["scan",*common])
            capture("explain",["explain","/model",*common])
            capture("doctor",["doctor",*common])
            before = root / "before.json"; after = root / "after.json"
            capture("snapshot",["snapshot",*common,"--output",str(before)])
            write(paths[2],{"model":"updated-model"})
            second = run(["snapshot",*common,"--output",str(after)])
            assert second.returncode == 0
            capture("diff",["diff",str(before),str(after)])
            token = next(r for r in report["resolutions"] if r["pointer"] == "/env/APP_TOKEN")
            captures["redaction"] = {"pointer":token["pointer"],"projection":token["observed_value"]}
            assert "synthetic-not-a-credential" not in json.dumps(report)
    data = {"binaryVersion":subprocess.check_output([str(BIN),"--version"],text=True).strip(),"binarySha256":hashlib.sha256(BIN.read_bytes()).hexdigest(),"normalization":"Only the ephemeral sandbox prefix is replaced by $SANDBOX. Commands retain every argument. stdout and stderr captured separately; no output elided.","isolation":"bubblewrap: read-only host, writable synthetic sandbox, empty /etc, network namespace; cleared HOME/XDG/environment, explicit project boundary and trusted simulated input context.","checks":checks,"commands":captures}
    (ROOT / "src/data/captures.json").write_text(json.dumps(data,indent=2)+"\n")
print(f"Verified {len(checks)} fixture winners against {data['binaryVersion']}; captured six commands and protected-value projection.")
