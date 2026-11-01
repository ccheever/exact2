// Interleaved rounds; wheel.swift supplies LLP 1142 D3's process-targeted gestures.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#include <signal.h>

static NSTask *runningApp, *runningWheel;
static volatile sig_atomic_t appPID, wheelPID;

static void interrupted(int number) {
    if (wheelPID) kill(wheelPID, SIGTERM);
    if (appPID) kill(appPID, SIGTERM);
    _exit(128 + number);
}

static void fail(NSString *why) {
    if (runningWheel.running) [runningWheel terminate];
    if (runningApp.running) [runningApp terminate];
    fprintf(stderr, "%s\n", why.UTF8String);
    exit(1);
}

static void until(double deadline) {
    while (CACurrentMediaTime() < deadline) [NSThread sleepForTimeInterval:MIN(0.02, deadline - CACurrentMediaTime())];
}

static NSDictionary *readJSON(NSString *path) {
    NSData *data = [NSData dataWithContentsOfFile:path];
    return data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL] : nil;
}

static NSDictionary *waitResult(NSString *path, NSTask *app, BOOL startup) {
    double deadline = CACurrentMediaTime() + 45;
    while (CACurrentMediaTime() < deadline) {
        NSDictionary *value = readJSON(path);
        if (value) return value;
        if (!app.running) {
            if (startup) return nil;
            fail([NSString stringWithFormat:@"app exited %d before %@", app.terminationStatus, path]);
        }
        [NSThread sleepForTimeInterval:0.02];
    }
    [app terminate];
    if (startup) return nil;
    fail([@"timed out: " stringByAppendingString:path]);
    return nil;
}

static NSDictionary *waitFile(NSString *path, NSTask *app) { return waitResult(path, app, NO); }

static void rejectStartup(NSTask *app, NSString *output, NSFileHandle *log, BOOL hook) {
    if (app.running) [app terminate];
    [app waitUntilExit];
    appPID = 0;
    NSDictionary *result = @{@"accepted": @NO, @"reason": @"startup readiness failed",
        @"exit": @(app.terminationStatus), @"pid": @(app.processIdentifier),
        @"requestedGpuHookEnabled": @(hook)};
    [[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingPrettyPrinted error:NULL]
        writeToFile:output atomically:YES];
    [log closeFile];
    fprintf(stdout, "%s discarded: startup readiness failed\n", output.UTF8String); fflush(stdout);
}

static NSTask *launch(NSString *path, NSArray *arguments, NSDictionary *environment, NSFileHandle *log) {
    NSTask *task = [NSTask new];
    task.executableURL = [NSURL fileURLWithPath:path];
    task.arguments = arguments;
    if (environment) task.environment = environment;
    task.standardOutput = log ?: NSFileHandle.fileHandleWithStandardOutput;
    task.standardError = log ?: NSFileHandle.fileHandleWithStandardError;
    NSError *error;
    if (![task launchAndReturnError:&error]) fail(error.description);
    return task;
}

static void activate(NSTask *app) {
    NSRunningApplication *application = [NSRunningApplication runningApplicationWithProcessIdentifier:app.processIdentifier];
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    [application activateWithOptions:NSApplicationActivateIgnoringOtherApps];
#pragma clang diagnostic pop
}

static BOOL focus(NSTask *app, NSString *output, NSMutableArray *checks) {
    double deadline = CACurrentMediaTime() + 5;
    NSString *path = [output stringByAppendingString:@".focus"];
    do {
        activate(app);
        [NSFileManager.defaultManager removeItemAtPath:path error:NULL];
        kill(app.processIdentifier, SIGUSR1);
        NSDictionary *state = waitFile(path, app);
        [checks addObject:state];
        if ([state[@"active"] boolValue] && [state[@"frontmostPid"] intValue] == app.processIdentifier
            && [state[@"keyWindow"] integerValue] == [state[@"window"] integerValue]) return YES;
        until(MIN(deadline, CACurrentMediaTime() + 0.05));
    } while (CACurrentMediaTime() < deadline);
    return NO;
}

static BOOL lostFocus(NSString *output) {
    return [readJSON([output stringByAppendingString:@".focus-losses"])[@"events"] count] > 0;
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        signal(SIGTERM, interrupted);
        signal(SIGINT, interrupted);
        if (argc < 5 || argc > 8) fail(@"measure <result-dir> <native-executable> <gpui-executable> <exact-wheel> [rounds=5] [first-round=1] [cold-only]; G3_PAINTED_BIN adds ink; G4_EGUI_BIN adds egui; BENCH_MOTION=bar|caret selects ten seconds without input or rest");
        NSString *hook = NSProcessInfo.processInfo.environment[@"G1_GPU_HOOK"] ?: @"0";
        if (![@[@"0", @"1"] containsObject:hook]) fail(@"measure: G1_GPU_HOOK must be 0 or 1 (empty is invalid)");
        NSString *motion = NSProcessInfo.processInfo.environment[@"BENCH_MOTION"];
        if (motion && ![@[@"bar", @"caret"] containsObject:motion]) fail(@"BENCH_MOTION must be bar or caret");
        NSString *workload = motion ?: @"fling";
        NSString *ink = NSProcessInfo.processInfo.environment[@"G3_PAINTED_BIN"];
        NSString *egui = NSProcessInfo.processInfo.environment[@"G4_EGUI_BIN"];
        BOOL onlyInk = [NSProcessInfo.processInfo.environment[@"G3_ONLY_INK"] boolValue];
        NSString *onlyArm = NSProcessInfo.processInfo.environment[@"G1_ONLY_ARM"];
        if (onlyArm && ![@[@"native", @"gpui", @"ink", @"egui"] containsObject:onlyArm])
            fail(@"G1_ONLY_ARM must name native, gpui, ink or egui");
        if (onlyInk && !ink.length) fail(@"G3_ONLY_INK needs G3_PAINTED_BIN");
        if (!CGPreflightPostEventAccess()) fail(@"this terminal lacks Accessibility permission for CGEvent.postToPid");
        NSString *directory = [@(argv[1]) stringByStandardizingPath];
        NSFileManager *files = NSFileManager.defaultManager;
        [files createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:NULL];
        int rounds = argc >= 6 ? atoi(argv[5]) : 5;
        int first = argc >= 7 ? atoi(argv[6]) : 1;
        BOOL coldOnly = argc == 8 && strcmp(argv[7], "cold-only") == 0;
        if (rounds < 1 || first < 1 || (argc == 8 && !coldOnly)) fail(@"invalid rounds, first round or cold-only argument");
        for (int round = first; round < first + rounds; round++) {
            NSMutableArray *targets = [NSMutableArray arrayWithArray:@[@"native", @"gpui"]];
            if (ink.length) [targets addObject:@"ink"];
            if (egui.length) [targets addObject:@"egui"];
            if (onlyInk) targets = [NSMutableArray arrayWithObject:@"ink"];
            if (onlyArm) {
                if (![targets containsObject:onlyArm]) fail(@"G1_ONLY_ARM executable missing");
                targets = [NSMutableArray arrayWithObject:onlyArm];
            }
            NSMutableArray *arms = [NSMutableArray array];
            for (NSUInteger i = 0; i < targets.count; i++) [arms addObject:targets[(round - 1 + i) % targets.count]];
            for (NSString *arm in arms) {
              // A launch cohort gates each original separately; never substitute a retry.
              for (int attempt = 0; attempt < (onlyArm ? 1 : 2); attempt++) {
                NSString *name = [NSString stringWithFormat:@"%@-%d%@", arm, round, attempt ? @"-retry" : @""];
                NSString *output = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".json"]];
                if ([files fileExistsAtPath:output] || [files fileExistsAtPath:[output stringByAppendingString:@".ready"]]) fail(@"use a fresh results directory");
                NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
                env[@"G1_OUTPUT"] = output; env[@"G1_ARM"] = arm; env[@"BENCH_LIVE"] = @"0";
                env[@"G1_GPU_HOOK"] = hook;
                env[@"EXACT_WINDOW_WIDTH"] = @"800";
                env[@"EXACT_WINDOW_HEIGHT"] = [arm isEqual:@"ink"] ? @"768" : @"800";
                [env removeObjectForKey:@"EXACT_AGENT"];
                [env removeObjectForKey:@"DYLD_INSERT_LIBRARIES"];
                [env removeObjectForKey:@"G1_COLD_ONLY"];
                for (NSString *key in @[@"EXACT_AGENT_SOCKET", @"BENCH_START_INDEX", @"EXACT_FONT",
                        @"EXACT_ROWS", @"EXACT_ASSETS", @"G1_TRACE", @"G4_DIAGNOSTICS", @"G1_ALLOW_IMAGE_REFUSALS"])
                    [env removeObjectForKey:key];
                if (coldOnly) env[@"G1_COLD_ONLY"] = @"1";
                NSString *logPath = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".log"]];
                [files createFileAtPath:logPath contents:nil attributes:nil];
                NSFileHandle *log = [NSFileHandle fileHandleForWritingAtPath:logPath];
                NSString *executable = [arm isEqual:@"egui"] ? egui : [arm isEqual:@"ink"] ? ink
                    : [arm isEqual:@"native"] ? @(argv[2]) : @(argv[3]);
                NSTask *app = launch(executable, @[], env, log);
                appPID = app.processIdentifier;
                runningApp = app;
                fprintf(stdout, "%s pid=%d\n", name.UTF8String, app.processIdentifier); fflush(stdout);
                activate(app);
                if (!waitResult([output stringByAppendingString:@".window"], app, YES)) {
                    rejectStartup(app, output, log, hook.boolValue);
                    continue;
                }
                NSMutableArray *checks = [NSMutableArray array];
                BOOL focused = focus(app, output, checks);
                NSDictionary *ready = waitResult([output stringByAppendingString:@".ready"], app, YES);
                if (!ready) {
                    rejectStartup(app, output, log, hook.boolValue);
                    continue;
                }
                if (motion && round == first && getenv("G5_CAPTURE") && focused && !lostFocus(output)) {
                    for (int frame = 0; frame < 2; frame++) {
                        NSString *capture = [output stringByAppendingFormat:@".%d.png", frame];
                        NSTask *shot = launch(@"/usr/sbin/screencapture", @[@"-x", @"-o", @"-l",
                            [ready[@"window"] stringValue], capture], nil, log);
                        [shot waitUntilExit];
                        if (shot.terminationStatus) fail(@"motion capture failed");
                        until(CACurrentMediaTime() + 0.5);
                    }
                }
                NSMutableArray *inputs = [NSMutableArray array];
                if (focused && !lostFocus(output) && !coldOnly) {
                    kill(app.processIdentifier, SIGUSR2);
                    NSDictionary *start = waitFile([output stringByAppendingFormat:@".%@", workload], app);
                    double began = [start[@"time"] doubleValue];
                    for (int fling = 0; !motion && fling < 4; fling++) {
                        until(began + fling * 2.5);
                        if (lostFocus(output)) break;
                        focused = focus(app, output, checks);
                        if (!focused || lostFocus(output)) break;
                        NSData *json = [NSJSONSerialization dataWithJSONObject:ready options:0 error:NULL];
                        double inputStart = CACurrentMediaTime();
                        NSTask *wheel = launch(@(argv[4]), @[[[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding]], nil, log);
                        runningWheel = wheel;
                        wheelPID = wheel.processIdentifier;
                        [wheel waitUntilExit];
                        wheelPID = 0;
                        [inputs addObject:@[@(inputStart), @(CACurrentMediaTime())]];
                        if (wheel.terminationStatus) fail(@"wheel helper failed");
                    }
                    if (focused && !lostFocus(output)) {
                        until(began + 10);
                        kill(app.processIdentifier, SIGUSR2);
                        NSDictionary *rest = waitFile([output stringByAppendingString:@".rest"], app);
                        if (!motion) until([rest[@"time"] doubleValue] + 10);
                    }
                }
                if (!focused || lostFocus(output)) {
                    NSDictionary *losses = readJSON([output stringByAppendingString:@".focus-losses"]);
                    NSDictionary *rejected = @{@"accepted": @NO, @"reason": @"focus", @"focusChecks": checks,
                        @"focusLosses": losses[@"events"] ?: @[], @"inputWindows": inputs,
                        @"requestedGpuHookEnabled": @(hook.boolValue)};
                    [[NSJSONSerialization dataWithJSONObject:rejected options:NSJSONWritingPrettyPrinted error:NULL]
                        writeToFile:output atomically:YES];
                    fprintf(stdout, "%s discarded: %s\n", name.UTF8String, [(losses ?: checks.lastObject) description].UTF8String);
                    fflush(stdout);
                    [app terminate]; [app waitUntilExit]; [log closeFile];
                    appPID = 0;
                    if (attempt) break;
                    continue;
                }
                kill(app.processIdentifier, SIGUSR2);
                NSMutableDictionary *result = [waitFile(output, app) mutableCopy];
                result[@"requestedGpuHookEnabled"] = @(hook.boolValue);
                result[@"inputWindows"] = inputs;
                result[@"focusChecks"] = checks;
                BOOL accepted = [result[@"activeAtStart"] boolValue] && [result[@"activeAtEnd"] boolValue]
                    && ![result[workload][@"inactiveTicks"] unsignedIntegerValue] && ![result[@"focusLosses"] count];
                result[@"accepted"] = @(accepted);
                [[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingPrettyPrinted error:NULL]
                    writeToFile:output atomically:YES];
                [app waitUntilExit];
                appPID = 0;
                [log closeFile];
                if (app.terminationStatus) fail(@"app failed after reporting");
                if (!accepted) {
                    fprintf(stdout, "%s discarded: %s\n", name.UTF8String, [result[@"focusLosses"] description].UTF8String);
                    fflush(stdout);
                    if (attempt) break;
                    continue;
                }
                if (motion) {
                    BOOL moved;
                    if ([arm isEqual:@"native"]) moved = [result[@"nativeOffsetEnd"] doubleValue] != 0;
                    else {
                        NSDictionary *offset = result[[arm stringByAppendingString:@"OffsetEnd"]];
                        moved = [offset[@"item"] intValue] != 0 || [offset[@"withinItem"] doubleValue] != 0;
                    }
                    if (moved || [result[workload][@"wheelEvents"] unsignedIntegerValue] != 0)
                        fail(@"static workload moved from row zero");
                }
                if (!motion && !coldOnly && [arm isEqual:@"native"] && [result[@"nativeOffsetEnd"] doubleValue] < 500) fail(@"native list did not scroll");
                if (!motion && !coldOnly && ([@[@"gpui", @"egui"] containsObject:arm])
                    && [result[[arm stringByAppendingString:@"OffsetEnd"]][@"item"] intValue] == 0)
                    fail([arm stringByAppendingString:@" list did not scroll"]);
                if (!motion && !coldOnly && [arm isEqual:@"ink"] && [result[@"inkOffsetEnd"][@"withinItem"] doubleValue] < 500) fail(@"ink list did not scroll");
                if ([result[@"thermalStart"] intValue] != [result[@"thermalEnd"] intValue]) fail(@"thermal state changed; retain this run but do not compare it");
                fprintf(stdout, "%s %s complete\n", name.UTF8String, workload.UTF8String); fflush(stdout);
                break;
              }
            }
        }
    }
}
