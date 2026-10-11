#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <string.h>

extern void ForceNativeEvent(int event);

static NSWindow *forceWindow;
static NSScreen *forceScreen;
static NSStatusItem *forceStatus;
static id globalMouse, localMouse, screenObserver, wakeObserver;
static NSTimer *pointerTimer;
static CGPathRef compactRegion, expandedRegion;
static BOOL isExpanded = NO;

static void onMain(void (^block)(void)) {
    if ([NSThread isMainThread]) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}

static NSRect unrestrictedFrame(id self, SEL selector, NSRect frame, NSScreen *screen) { return frame; }
static BOOL cannotBecomeMain(id self, SEL selector) { return NO; }

static NSScreen *targetScreen(void) {
    for (NSScreen *screen in NSScreen.screens) {
        if (screen.safeAreaInsets.top > 0) return screen;
    }
    for (NSScreen *screen in NSScreen.screens) {
        if ([screen isEqual:forceScreen]) return screen;
    }
    return NSScreen.mainScreen ?: NSScreen.screens.firstObject;
}

static NSDictionary *geometry(void) {
    forceScreen = targetScreen();
    CGFloat notchWidth = 0;
    BOOL hasNotch = forceScreen.safeAreaInsets.top > 0;
    if (hasNotch) {
        NSRect left = forceScreen.auxiliaryTopLeftArea;
        NSRect right = forceScreen.auxiliaryTopRightArea;
        notchWidth = MAX(0, NSMinX(right)-NSMaxX(left));
    }
    CGFloat width = MIN(432, forceScreen.frame.size.width-24);
    CGFloat expandedHeight = MIN(480, forceScreen.visibleFrame.size.height-20);
    // All content begins below the menu bar. The compact strip follows the camera's
    // lower edge without occupying either menu-bar wing or covering status items.
    return @{ @"gap":@0, @"top":@(hasNotch ? 22 : 28), @"hasNotch":@(hasNotch),
              @"collapsedWidth":@(hasNotch ? MAX(176, notchWidth) : 224),
              @"expandedWidth":@(width), @"expandedHeight":@(expandedHeight) };
}

static CGPathRef interactionRegion(CGFloat canvas, CGFloat width, CGFloat height, CGFloat neck, BOOL expanded, BOOL hasNotch) {
    CGFloat origin = (canvas-width)/2;
    if (!hasNotch) return CGPathCreateWithRoundedRect(CGRectMake(origin, 0, width, height), expanded ? 23 : 14, expanded ? 23 : 14, NULL);
    CGFloat left = origin+(width-neck)/2, right = origin+(width+neck)/2;
    CGFloat edge = origin+width, shoulder = expanded ? 18 : 0, spread = expanded ? 16 : 0;
    CGFloat corner = expanded ? 22 : 0, bottom = expanded ? 23 : MIN(12, height/2);
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, left, 0);
    CGPathAddLineToPoint(path, NULL, right, 0);
    CGPathAddCurveToPoint(path, NULL, right, shoulder, right, shoulder, right+spread, shoulder);
    CGPathAddLineToPoint(path, NULL, edge-corner, shoulder);
    CGPathAddQuadCurveToPoint(path, NULL, edge, shoulder, edge, shoulder+corner);
    CGPathAddLineToPoint(path, NULL, edge, height-bottom);
    CGPathAddQuadCurveToPoint(path, NULL, edge, height, edge-bottom, height);
    CGPathAddLineToPoint(path, NULL, origin+bottom, height);
    CGPathAddQuadCurveToPoint(path, NULL, origin, height, origin, height-bottom);
    CGPathAddLineToPoint(path, NULL, origin, shoulder+corner);
    CGPathAddQuadCurveToPoint(path, NULL, origin, shoulder, origin+corner, shoulder);
    CGPathAddLineToPoint(path, NULL, left-spread, shoulder);
    CGPathAddCurveToPoint(path, NULL, left, shoulder, left, shoulder, left, 0);
    CGPathCloseSubpath(path);
    return path;
}

static void updatePointerInteraction(void) {
    if (!forceWindow || !compactRegion || !expandedRegion) return;
    NSPoint point = NSEvent.mouseLocation;
    NSRect frame = forceWindow.frame;
    CGPoint local = CGPointMake(point.x-NSMinX(frame), NSMaxY(frame)-point.y);
    BOOL inside = CGPathContainsPoint(isExpanded ? expandedRegion : compactRegion, NULL, local, NO);
    forceWindow.ignoresMouseEvents = !inside;
}

static void placeWindow(void) {
    if (!forceWindow) return;
    NSDictionary *g = geometry();
    // Keep a stable transparent canvas so the webview can animate a single outline.
    // Resizing NSWindow at the state boundary would cut off that animation.
    CGFloat width = [g[@"expandedWidth"] doubleValue];
    CGFloat height = [g[@"expandedHeight"] doubleValue];
    CGFloat compact = [g[@"collapsedWidth"] doubleValue];
    if (compactRegion) CGPathRelease(compactRegion);
    if (expandedRegion) CGPathRelease(expandedRegion);
    compactRegion = interactionRegion(width, compact, [g[@"top"] doubleValue], compact, NO, [g[@"hasNotch"] boolValue]);
    expandedRegion = interactionRegion(width, width, height, compact, YES, [g[@"hasNotch"] boolValue]);
    NSRect screenFrame = forceScreen.frame;
    CGFloat topY = MIN(NSMaxY(screenFrame)-forceScreen.safeAreaInsets.top, NSMaxY(forceScreen.visibleFrame));
    if (![g[@"hasNotch"] boolValue]) topY -= 4;
    NSRect frame = NSMakeRect(NSMidX(screenFrame)-width/2, topY-height, width, height);
    [forceWindow setFrame:frame display:YES animate:NO];
    [forceWindow orderFrontRegardless];
}

@interface ForceTray : NSObject
- (void)show:(id)sender;
- (void)quit:(id)sender;
@end
@implementation ForceTray
- (void)show:(id)sender { ForceNativeEvent(1); }
- (void)quit:(id)sender { ForceNativeEvent(4); }
@end
static ForceTray *trayTarget;

char *ForceConfigure(void) {
    __block char *result;
    onMain(^{
        for (NSWindow *window in NSApp.windows) {
            if ([window.title isEqualToString:@"Force"]) { forceWindow=window; break; }
        }
        if (!forceWindow) forceWindow=NSApp.windows.firstObject;
        Class base = object_getClass(forceWindow);
        // Keep AppKit's existing runtime class and computed-property metadata intact.
        // These overrides apply only to WailsWindow in this application's process.
        Method method = class_getInstanceMethod(base, @selector(constrainFrameRect:toScreen:));
        class_addMethod(base, @selector(constrainFrameRect:toScreen:), (IMP)unrestrictedFrame, method_getTypeEncoding(method));
        class_addMethod(base, @selector(canBecomeMainWindow), (IMP)cannotBecomeMain, "c@:");
        forceWindow.backgroundColor = NSColor.clearColor;
        forceWindow.opaque = NO;
        forceWindow.hasShadow = NO;
        forceWindow.level = NSStatusWindowLevel+1;
        forceWindow.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle;
        if (@available(macOS 13.0, *)) forceWindow.collectionBehavior |= NSWindowCollectionBehaviorCanJoinAllApplications;
        forceWindow.movableByWindowBackground = NO;
        forceWindow.acceptsMouseMovedEvents = YES;
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        placeWindow();

        // Transparent space must pass clicks through to the application underneath.
        // Cursor sampling also works while this window ignores mouse events.
        pointerTimer = [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *timer) { updatePointerInteraction(); }];
        pointerTimer.tolerance = 0.03;

        trayTarget = [ForceTray new];
        forceStatus = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        NSImage *image = [NSImage imageWithSystemSymbolName:@"timer" accessibilityDescription:@"Force"];
        image.template=YES;
        forceStatus.button.image=image;
        forceStatus.button.toolTip=@"Force · 把时间放在眼前";
        NSMenu *menu = [NSMenu new];
        NSMenuItem *show=[menu addItemWithTitle:@"展开 Force" action:@selector(show:) keyEquivalent:@""];
        show.target=trayTarget;
        [menu addItem:NSMenuItem.separatorItem];
        NSMenuItem *quit=[menu addItemWithTitle:@"退出 Force（计时继续）" action:@selector(quit:) keyEquivalent:@""];
        quit.target=trayTarget;
        forceStatus.menu=menu;

        globalMouse = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown|NSEventMaskRightMouseDown|NSEventMaskMouseMoved handler:^(NSEvent *event) {
            if (event.type == NSEventTypeMouseMoved) { updatePointerInteraction(); return; }
            if (isExpanded) ForceNativeEvent(2);
        }];
        localMouse = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown|NSEventMaskKeyDown|NSEventMaskMouseMoved handler:^NSEvent *(NSEvent *event) {
            if (event.type == NSEventTypeMouseMoved) updatePointerInteraction();
            if (isExpanded && ((event.type == NSEventTypeLeftMouseDown && event.window != forceWindow) || (event.type == NSEventTypeKeyDown && event.keyCode == 53))) {
                ForceNativeEvent(2);
                if (event.type == NSEventTypeKeyDown) return nil;
            }
            return event;
        }];
        screenObserver = [[NSNotificationCenter defaultCenter] addObserverForName:NSApplicationDidChangeScreenParametersNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            placeWindow(); ForceNativeEvent(3);
        }];
        wakeObserver = [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidWakeNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            placeWindow(); ForceNativeEvent(5);
        }];
        NSData *json=[NSJSONSerialization dataWithJSONObject:geometry() options:0 error:nil];
        result=strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
    });
    return result;
}

char *ForceGeometry(void) {
    __block char *result;
    onMain(^{
        NSData *json=[NSJSONSerialization dataWithJSONObject:geometry() options:0 error:nil];
        result=strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
    });
    return result;
}

void ForceExpand(int expanded) {
    dispatch_async(dispatch_get_main_queue(), ^{
        isExpanded = expanded != 0;
        placeWindow();
        if (isExpanded) { [NSApp activateIgnoringOtherApps:YES]; [forceWindow makeKeyAndOrderFront:nil]; }
        else [forceWindow resignKeyWindow];
    });
}

void ForceShutdown(void) {
    onMain(^{
        if (globalMouse) [NSEvent removeMonitor:globalMouse];
        if (localMouse) [NSEvent removeMonitor:localMouse];
        if (screenObserver) [[NSNotificationCenter defaultCenter] removeObserver:screenObserver];
        if (wakeObserver) [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:wakeObserver];
        [pointerTimer invalidate];
        if (compactRegion) { CGPathRelease(compactRegion); compactRegion=NULL; }
        if (expandedRegion) { CGPathRelease(expandedRegion); expandedRegion=NULL; }
        if (forceStatus) [[NSStatusBar systemStatusBar] removeStatusItem:forceStatus];
    });
}
