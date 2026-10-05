import pathlib, subprocess, tempfile
root=pathlib.Path.cwd()
here=pathlib.Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="t3-timeline-keyboard-") as temp:
    binary=pathlib.Path(temp)/"keyboard"
    sources=sorted((root/"host/apple/Sources/ExactKit").rglob("*.swift"))
    subprocess.run(["xcrun","swiftc","-swift-version","5","-module-name","ExactKit","-I",str(root/"host/apple/Sources/CExact"),*map(str,sources),str(here/"main.swift"),"-L",str(root/"target/aarch64-apple-darwin/apple-dev"),"-lmacos_t3_code_apple","-lc++","-o",str(binary)],check=True)
    subprocess.run([str(binary)],check=True)
