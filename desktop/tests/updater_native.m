// Synthetic lifecycle checks. No account access, network, or real app replacement.
#import "../native/updater.m"
#include <assert.h>
int main(void) {
    @autoreleasepool {
        delegate = [WBUpdateDelegate new];
        wb_updater_set_busy(true);
        assert(![delegate updater:nil mayPerformUpdateCheck:SPUUpdateCheckUpdates error:nil]);
        assert(![delegate updaterShouldRelaunchApplication:nil]);
        __block int installed = 0;
        assert([delegate updater:nil shouldPostponeRelaunchForUpdate:nil untilInvokingBlock:^{ installed++; }]);
        assert(installed == 0);
        wb_updater_set_busy(true);
        assert(installed == 0);
        wb_updater_set_busy(false);
        assert(installed == 1 && wb_updater_installing());
        wb_updater_set_busy(false);
        assert(installed == 1); // Never resume the same install twice.
        assert(![delegate updater:nil mayPerformUpdateCheck:SPUUpdateCheckUpdates error:nil]);
        [delegate updater:nil didAbortWithError:nil];
        assert(!wb_updater_installing());
        assert([delegate updater:nil mayPerformUpdateCheck:SPUUpdateCheckUpdates error:nil]);
        assert([delegate updaterShouldRelaunchApplication:nil]);
        assert(![delegate updater:nil shouldPostponeRelaunchForUpdate:nil untilInvokingBlock:^{}]);
        assert(wb_updater_installing());
        puts("Native updater lifecycle: busy deferral, single resume, abort recovery, idle install passed.");
    }
    return 0;
}
