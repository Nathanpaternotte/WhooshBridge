// Isolated test app: no Whoosh Bridge workers, accounts, FIT files or preferences.
#import <Cocoa/Cocoa.h>
#include <stdbool.h>
extern bool wb_updater_start(void);
extern void wb_updater_set_busy(bool);
extern bool wb_updater_can_check(void);
extern void wb_updater_check(void);

@interface TestDelegate : NSObject <NSApplicationDelegate>
@end
@implementation TestDelegate
- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    NSString *version = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"CFBundleVersion"];
    NSString *marker = [[NSBundle mainBundle].bundlePath.stringByDeletingLastPathComponent stringByAppendingPathComponent:@"launched-version.txt"];
    [version writeToFile:marker atomically:YES encoding:NSUTF8StringEncoding error:nil];
    if ([version isEqualToString:@"2"]) { [NSApp terminate:nil]; return; }
    if (!wb_updater_start()) { exit(2); }
    wb_updater_set_busy(false);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        if (!wb_updater_can_check()) { exit(3); }
        wb_updater_check();
    });
}
@end
int main(void) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        TestDelegate *delegate = [TestDelegate new];
        app.delegate = delegate;
        [app setActivationPolicy:NSApplicationActivationPolicyRegular];
        [app run];
    }
    return 0;
}
