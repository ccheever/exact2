import os, pathlib, subprocess, tempfile
root=pathlib.Path.cwd()
x=subprocess.check_output(["xcode-select","-p"],text=True).strip()
f=x+"/Platforms/MacOSX.platform/Developer/Library/Frameworks"
l=x+"/Platforms/MacOSX.platform/Developer/usr/lib"
out=root/"target/t3-verify-activity"
out.mkdir(parents=True,exist_ok=True)
subprocess.run(["/Users/daehyeonmun/.bun-1.4.2/bin/bun","-e",'import {writeDataKeys} from "./host/apple/data-keys.mjs"; import manifest from "./examples/macos/t3-code/app.json"; writeDataKeys({manifest}, "target/t3-verify-activity/ExactDataKeys.swift");'],check=True)
cmd=["xcrun","swiftc","-swift-version","5","-module-name","T3ActivityTests","-F",f,"-I",l,"-L",l,"-Xlinker","-rpath","-Xlinker",f,"-Xlinker","-rpath","-Xlinker",l,"host/apple/modules/ExactNativeModule.swift",str(out/"ExactDataKeys.swift")]+[str(p) for p in sorted(pathlib.Path("examples/macos/t3-code/modules/apple").glob("*.swift"))]+["examples/macos/t3-code/apple/tests/activity/main.swift","-o",str(out/"activity-tests")]
subprocess.run(cmd,check=True)
subprocess.run([str(out/"activity-tests")],check=True)
