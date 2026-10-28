// Five interleaved rounds; wheel.swift supplies LLP 1142 D3's process-targeted gestures.
#import <Foundation/Foundation.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#include <signal.h>

static void fail(NSString *why) {
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

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc < 5 || argc > 6) fail(@"measure <result-dir> <native-executable> <gpui-executable> <exact-wheel> [rounds=5]");
        if (!CGPreflightPostEventAccess()) fail(@"this terminal lacks Accessibility permission for CGEvent.postToPid");
        NSString *directory = [@(argv[1]) stringByStandardizingPath];
        NSFileManager *files = NSFileManager.defaultManager;
        [files createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:NULL];
        int rounds = argc == 6 ? atoi(argv[5]) : 5;
        for (int round = 1; round <= rounds; round++) {
            NSArray *arms = round % 2 ? @[@"native", @"gpui"] : @[@"gpui", @"native"];
            for (NSString *arm in arms) {
                NSString *name = [NSString stringWithFormat:@"%@-%d", arm, round];
                NSString *output = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".json"]];
                if ([files fileExistsAtPath:output] || [files fileExistsAtPath:[output stringByAppendingString:@".ready"]]) fail(@"use a fresh results directory");
                NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
                env[@"G1_OUTPUT"] = output; env[@"G1_ARM"] = arm; env[@"BENCH_LIVE"] = @"0";
                env[@"EXACT_WINDOW_WIDTH"] = @"800"; env[@"EXACT_WINDOW_HEIGHT"] = @"800";
                [env removeObjectForKey:@"EXACT_AGENT"];
                [env removeObjectForKey:@"DYLD_INSERT_LIBRARIES"];
                NSString *logPath = [directory stringByAppendingPathComponent:[name stringByAppendingString:@".log"]];
                [files createFileAtPath:logPath contents:nil attributes:nil];
                NSFileHandle *log = [NSFileHandle fileHandleForWritingAtPath:logPath];
                NSTask *app = launch([arm isEqual:@"native"] ? @(argv[2]) : @(argv[3]), @[], env, log);
                fprintf(stdout, "%s pid=%d\n", name.UTF8String, app.processIdentifier); fflush(stdout);
                NSDictionary *ready = waitFile([output stringByAppendingString:@".ready"], app);
                kill(app.processIdentifier, SIGUSR2);
                NSDictionary *start = waitFile([output stringByAppendingString:@".fling"], app);
                double began = [start[@"time"] doubleValue];
                NSMutableArray *inputs = [NSMutableArray array];
                for (int fling = 0; fling < 4; fling++) {
                    until(began + fling * 2.5);
                    NSData *json = [NSJSONSerialization dataWithJSONObject:ready options:0 error:NULL];
                    double inputStart = CACurrentMediaTime();
                    NSTask *wheel = launch(@(argv[4]), @[[[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding]], nil, log);
                    [wheel waitUntilExit];
                    [inputs addObject:@[@(inputStart), @(CACurrentMediaTime())]];
                    if (wheel.terminationStatus) { [app terminate]; fail(@"wheel helper failed"); }
                }
                until(began + 10);
                kill(app.processIdentifier, SIGUSR2);
                NSDictionary *rest = waitFile([output stringByAppendingString:@".rest"], app);
                until([rest[@"time"] doubleValue] + 10);
                kill(app.processIdentifier, SIGUSR2);
                NSMutableDictionary *result = [waitFile(output, app) mutableCopy];
                result[@"inputWindows"] = inputs;
                [[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingPrettyPrinted error:NULL]
                    writeToFile:output atomically:YES];
                [app waitUntilExit];
                [log closeFile];
                if (app.terminationStatus) fail(@"app failed after reporting");
                if ([arm isEqual:@"native"] && [result[@"nativeOffsetEnd"] doubleValue] < 500) fail(@"native list did not scroll");
                if ([arm isEqual:@"gpui"] && [result[@"gpuiOffsetEnd"][@"item"] intValue] == 0) fail(@"GPUI list did not scroll");
                if (![result[@"activeAtStart"] boolValue] || ![result[@"activeAtEnd"] boolValue]
                    || [result[@"fling"][@"inactiveTicks"] unsignedIntegerValue]) fail(@"window lost focus; retain this run but do not compare it");
                if ([result[@"thermalStart"] intValue] != [result[@"thermalEnd"] intValue]) fail(@"thermal state changed; retain this run but do not compare it");
                fprintf(stdout, "%s cold=%.3f ms\n", name.UTF8String, [result[@"coldMs"] doubleValue]); fflush(stdout);
            }
        }
    }
}
