// Main-thread-only adapter between egui and Sparkle's standard macOS UI.
#import <Cocoa/Cocoa.h>
#import <Sparkle/Sparkle.h>

@interface WBUpdateDelegate : NSObject <SPUUpdaterDelegate>
@property BOOL busy;
@property BOOL installing;
@property(copy) void (^pendingInstall)(void);
@end
@implementation WBUpdateDelegate
- (BOOL)updater:(SPUUpdater *)updater mayPerformUpdateCheck:(SPUUpdateCheck)check error:(NSError **)error {
    return !self.busy && !self.installing;
}
- (BOOL)updater:(SPUUpdater *)updater shouldPostponeRelaunchForUpdate:(SUAppcastItem *)item untilInvokingBlock:(void (^)(void))handler {
    if (self.busy) {
        self.pendingInstall = handler;
        return YES;
    }
    self.installing = YES;
    return NO;
}
- (BOOL)updaterShouldRelaunchApplication:(SPUUpdater *)updater {
    return !self.busy;
}
- (void)updater:(SPUUpdater *)updater didAbortWithError:(NSError *)error {
    self.installing = NO;
    self.pendingInstall = nil;
}
@end

static SPUStandardUpdaterController *controller;
static WBUpdateDelegate *delegate;

bool wb_updater_start(void) {
    NSCAssert([NSThread isMainThread], @"Updater must run on main thread");
    if (controller) return true;
    delegate = [WBUpdateDelegate new];
    delegate.busy = YES;
    controller = [[SPUStandardUpdaterController alloc] initWithStartingUpdater:NO updaterDelegate:delegate userDriverDelegate:nil];
    NSError *error = nil;
    if (![controller.updater startUpdater:&error]) {
        controller = nil;
        delegate = nil;
        return false; // Do not log raw errors or service responses.
    }
    return true;
}
void wb_updater_set_busy(bool busy) {
    delegate.busy = busy;
    if (!busy && delegate.pendingInstall) {
        void (^handler)(void) = delegate.pendingInstall;
        delegate.pendingInstall = nil;
        delegate.installing = YES;
        handler();
    }
}
bool wb_updater_installing(void) { return delegate.installing; }
bool wb_updater_can_check(void) {
    return controller && !delegate.busy && !delegate.installing && controller.updater.canCheckForUpdates;
}
void wb_updater_check(void) {
    if (wb_updater_can_check()) [controller checkForUpdates:nil];
}
bool wb_updater_automatic(void) { return controller.updater.automaticallyChecksForUpdates; }
void wb_updater_set_automatic(bool enabled) { controller.updater.automaticallyChecksForUpdates = enabled; }
