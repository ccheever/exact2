import pathlib, subprocess, sys, tempfile
root = pathlib.Path.cwd()
evidence = pathlib.Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="t3-timeline-hooks-") as scratch:
    binary = pathlib.Path(scratch) / "hooks"
    sources = [evidence / "Stub.swift", root / "examples/macos/t3-code/modules/apple/T3ToolActivityIcon.swift", root / "examples/macos/t3-code/modules/apple/T3TimelineTooltip.swift", evidence / "main.swift"]
    subprocess.run(["xcrun", "swiftc", "-swift-version", "5", *map(str, sources), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
