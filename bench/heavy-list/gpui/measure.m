// Five interleaved rounds; wheel.swift supplies LLP 1142 D3's process-targeted gestures.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#include <signal.h>

static NSTask *runningApp, *runningWheel;

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

static NSDictionary *waitFile(NSString *path, NSTask *app) {
    double deadline = CACurrentMediaTime() + 45;
    while (CACurrentMediaTime() < deadline) {
        NSDictionary *value = readJSON(path);
        if (value) return value;
        if (!app.running) fail([NSString stringWithFormat:@"app exited %d before %@", app.terminationStatus, path]);
        [NSThread sleepForTimeInterval:0.02];
    }
    [app terminate];
    fail([@"timed out: " stringByAppendingString:path]);
    return nil;
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
        if (argc < 5 || argc > 8) fail(@"measure <result-dir> <native-executable> <gpui-executable> <exact-wheel> [rounds=5] [first-round=1] [cold-only]");
        if (!CGPreflightPostEventAccess()) fail(@"this terminal lacks Accessibility permission for CGEvent.postToPid");
        NSString *directory = [@(argv[1]) stringByStandardizingPath];
        NSFileManager *files = NSFileManager.defaultManager;
        [files createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:NULL];
        int rounds = argc >= 6 ? atoi(argv[5]) : 5;
        int first = argc >= 7 ? atoi(argv[6]) : 1;
        BOOL coldOnly = argc == 8 && strcmp(argv[7], "cold-only") == 0;
        if (rounds < 1 || first < 1 || (argc == 8 && !coldOnly)) fail(@"invalid rounds, first round or cold-only argument");
        for (int round = first; round < first + rounds; round++) {
            NSArray *arms = round % 2 ? @[@"native", @"gpui"] : @[@"gpui", @"native"];
            for (NSString *arm in arms) {
              for (int attempt = 0; attempt < 2; attempt++) {
                NSString *name = [NSString stringWithFormat:@"%@-%d%@", arm, round, attempt ? @"-retry" : @""];
                NSString *output = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".json"]];
                if ([files fileExistsAtPath:output] || [files fileExistsAtPath:[output stringByAppendingString:@".ready"]]) fail(@"use a fresh results directory");
                NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
                env[@"G1_OUTPUT"] = output; env[@"G1_ARM"] = arm; env[@"BENCH_LIVE"] = @"0";
                env[@"EXACT_WINDOW_WIDTH"] = @"800"; env[@"EXACT_WINDOW_HEIGHT"] = @"800";
                [env removeObjectForKey:@"EXACT_AGENT"];
                [env removeObjectForKey:@"DYLD_INSERT_LIBRARIES"];
                [env removeObjectForKey:@"G1_COLD_ONLY"];
                if (coldOnly) env[@"G1_COLD_ONLY"] = @"1";
                NSString *logPath = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".log"]];
                [files createFileAtPath:logPath contents:nil attributes:nil];
                NSFileHandle *log = [NSFileHandle fileHandleForWritingAtPath:logPath];
                NSTask *app = launch([arm isEqual:@"native"] ? @(argv[2]) : @(argv[3]), @[], env, log);
                runningApp = app;
                fprintf(stdout, "%s pid=%d\n", name.UTF8String, app.processIdentifier); fflush(stdout);
                activate(app);
                waitFile([output stringByAppendingString:@".window"], app);
                NSMutableArray *checks = [NSMutableArray array];
                BOOL focused = focus(app, output, checks);
                NSDictionary *ready = waitFile([output stringByAppendingString:@".ready"], app);
                NSMutableArray *inputs = [NSMutableArray array];
                if (focused && !lostFocus(output) && !coldOnly) {
                    kill(app.processIdentifier, SIGUSR2);
                    NSDictionary *start = waitFile([output stringByAppendingString:@".fling"], app);
                    double began = [start[@"time"] doubleValue];
                    for (int fling = 0; fling < 4; fling++) {
                        until(began + fling * 2.5);
                        if (lostFocus(output)) break;
                        focused = focus(app, output, checks);
                        if (!focused || lostFocus(output)) break;
                        NSData *json = [NSJSONSerialization dataWithJSONObject:ready options:0 error:NULL];
                        double inputStart = CACurrentMediaTime();
                        NSTask *wheel = launch(@(argv[4]), @[[[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding]], nil, log);
                        runningWheel = wheel;
                        [wheel waitUntilExit];
                        [inputs addObject:@[@(inputStart), @(CACurrentMediaTime())]];
                        if (wheel.terminationStatus) fail(@"wheel helper failed");
                    }
                    if (focused && !lostFocus(output)) {
                        until(began + 10);
                        kill(app.processIdentifier, SIGUSR2);
                        NSDictionary *rest = waitFile([output stringByAppendingString:@".rest"], app);
                        until([rest[@"time"] doubleValue] + 10);
                    }
                }
                if (!focused || lostFocus(output)) {
                    NSDictionary *losses = readJSON([output stringByAppendingString:@".focus-losses"]);
                    NSDictionary *rejected = @{@"accepted": @NO, @"reason": @"focus", @"focusChecks": checks,
                        @"focusLosses": losses[@"events"] ?: @[], @"inputWindows": inputs};
                    [[NSJSONSerialization dataWithJSONObject:rejected options:NSJSONWritingPrettyPrinted error:NULL]
                        writeToFile:output atomically:YES];
                    fprintf(stdout, "%s discarded: %s\n", name.UTF8String, [(losses ?: checks.lastObject) description].UTF8String);
                    fflush(stdout);
                    [app terminate]; [app waitUntilExit]; [log closeFile];
                    if (attempt) break;
                    continue;
                }
                kill(app.processIdentifier, SIGUSR2);
                NSMutableDictionary *result = [waitFile(output, app) mutableCopy];
                result[@"inputWindows"] = inputs;
                result[@"focusChecks"] = checks;
                BOOL accepted = [result[@"activeAtStart"] boolValue] && [result[@"activeAtEnd"] boolValue]
                    && ![result[@"fling"][@"inactiveTicks"] unsignedIntegerValue] && ![result[@"focusLosses"] count];
                result[@"accepted"] = @(accepted);
                [[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingPrettyPrinted error:NULL]
                    writeToFile:output atomically:YES];
                [app waitUntilExit];
                [log closeFile];
                if (app.terminationStatus) fail(@"app failed after reporting");
                if (!accepted) {
                    fprintf(stdout, "%s discarded: %s\n", name.UTF8String, [result[@"focusLosses"] description].UTF8String);
                    fflush(stdout);
                    if (attempt) break;
                    continue;
                }
                if (!coldOnly && [arm isEqual:@"native"] && [result[@"nativeOffsetEnd"] doubleValue] < 500) fail(@"native list did not scroll");
                if (!coldOnly && [arm isEqual:@"gpui"] && [result[@"gpuiOffsetEnd"][@"item"] intValue] == 0) fail(@"GPUI list did not scroll");
                if ([result[@"thermalStart"] intValue] != [result[@"thermalEnd"] intValue]) fail(@"thermal state changed; retain this run but do not compare it");
                fprintf(stdout, "%s cold=%.3f ms\n", name.UTF8String, [result[@"coldMs"] doubleValue]); fflush(stdout);
                break;
              }
            }
        }
    }
}
