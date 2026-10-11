#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <string.h>

extern void ForceNativeEvent(int event);
extern void ForceDetailRequest(char *payload);

static NSWindow *forceWindow, *detailWindow;
static WKWebView *detailWebView;
static NSStatusItem *forceStatus;
static id globalMouse, localMouse, screenObserver, wakeObserver, moveObserver;
static BOOL isExpanded = NO, placed = NO, detailReady = NO;
static NSString *detailView = @"history";
static BOOL dragging = NO;
static NSPoint dragStart, dragOrigin;

static void onMain(void (^block)(void)) {
    if ([NSThread isMainThread]) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}
static BOOL canBecomeMain(id self, SEL selector) { return YES; }
static NSTimeInterval resizeDuration(id self, SEL selector, NSRect frame) { return 0.25; }

static NSScreen *targetScreen(void) {
    if (placed) {
        NSRect frame = forceWindow.frame;
        NSPoint point = NSMakePoint(NSMaxX(frame)-140, NSMaxY(frame)-27);
        for (NSScreen *screen in NSScreen.screens) if (NSPointInRect(point,screen.frame)) return screen;
    }
    return NSScreen.mainScreen ?: NSScreen.screens.firstObject;
}
static NSDictionary *geometry(void) {
    NSRect visible = targetScreen().visibleFrame;
    return @{ @"compactWidth":@280, @"compactHeight":@54,
              @"panelWidth":@(MIN(368,visible.size.width-24)),
              @"panelHeight":@(MIN(456,visible.size.height-24)) };
}
static void placeWindow(BOOL reset) {
    if (!forceWindow) return;
    NSDictionary *g = geometry();
    CGFloat width=[g[isExpanded ? @"panelWidth" : @"compactWidth"] doubleValue];
    CGFloat height=[g[isExpanded ? @"panelHeight" : @"compactHeight"] doubleValue];
    NSRect visible=targetScreen().visibleFrame;
    CGFloat right=NSMaxX(visible)-24, top=NSMaxY(visible)-24;
    if (!reset && placed) { right=NSMaxX(forceWindow.frame); top=NSMaxY(forceWindow.frame); }
    else if (!reset && [NSUserDefaults.standardUserDefaults boolForKey:@"force.float.position"]) {
        right=[NSUserDefaults.standardUserDefaults doubleForKey:@"force.float.right"];
        top=[NSUserDefaults.standardUserDefaults doubleForKey:@"force.float.top"];
        NSPoint saved=NSMakePoint(right-140,top-27);
        for (NSScreen *screen in NSScreen.screens) if (NSPointInRect(saved,screen.frame)) { visible=screen.visibleFrame;break; }
    }
    right=MIN(MAX(right,NSMinX(visible)+width+12),NSMaxX(visible)-12);
    top=MIN(MAX(top,NSMinY(visible)+height+12),NSMaxY(visible)-12);
    // The native window follows the visible surface, so the compact bar never
    // leaves a large invisible canvas above other applications.
    BOOL animate=placed && !reset && (forceWindow.frame.size.width!=width || forceWindow.frame.size.height!=height);
    [forceWindow setFrame:NSMakeRect(right-width,top-height,width,height) display:YES animate:animate];
    placed=YES;
}
static WKWebView *findWebView(NSView *view) {
    if ([view isKindOfClass:WKWebView.class]) return (WKWebView *)view;
    for (NSView *child in view.subviews) { WKWebView *found=findWebView(child);if(found)return found; }
    return nil;
}
static void notifyDetailView(void) {
    if (!detailReady) return;
    NSString *script=[NSString stringWithFormat:@"window.dispatchEvent(new CustomEvent('force:detail-view',{detail:'%@'}))",detailView];
    [detailWebView evaluateJavaScript:script completionHandler:nil];
}
@interface ForceDesktop : NSObject <WKScriptMessageHandler,WKNavigationDelegate,NSWindowDelegate>
- (void)show:(id)sender;
- (void)reset:(id)sender;
- (void)quit:(id)sender;
@end
@implementation ForceDesktop
- (void)show:(id)sender { [forceWindow orderFrontRegardless];ForceNativeEvent(1); }
- (void)reset:(id)sender { placeWindow(YES);[self show:sender]; }
- (void)quit:(id)sender { ForceNativeEvent(4); }
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
    NSURL *url=message.frameInfo.request.URL;
    // The secondary window has a separate, read-only bridge. It never forwards
    // Wails' main-window commands or mutation calls to the primary webview.
    if (![message.name isEqualToString:@"forceDetails"] || message.webView!=detailWebView || !message.frameInfo.mainFrame ||
        ![url.scheme isEqualToString:@"wails"] || ![url.host isEqualToString:@"wails"] || ![message.body isKindOfClass:NSString.class]) return;
    ForceDetailRequest((char *)[(NSString *)message.body UTF8String]);
}
- (void)webView:(WKWebView *)webView didFinishNavigation:(WKNavigation *)navigation { detailReady=YES;notifyDetailView(); }
- (void)webView:(WKWebView *)webView decidePolicyForNavigationAction:(WKNavigationAction *)action decisionHandler:(void (^)(WKNavigationActionPolicy))decisionHandler {
    NSURL *url=action.request.URL;
    decisionHandler([url.scheme isEqualToString:@"wails"] && [url.host isEqualToString:@"wails"] ? WKNavigationActionPolicyAllow : WKNavigationActionPolicyCancel);
}
- (BOOL)windowShouldClose:(NSWindow *)sender { [sender orderOut:nil];return NO; }
@end
static ForceDesktop *desktopTarget;

char *ForceConfigure(void) {
    __block char *result;
    onMain(^{
        for (NSWindow *window in NSApp.windows) if ([window.title isEqualToString:@"Force"]) { forceWindow=window;break; }
        if (!forceWindow) forceWindow=NSApp.windows.firstObject;
        Class base=object_getClass(forceWindow);
        // Preserve WailsWindow's runtime class and AppKit's computed metadata.
        class_addMethod(base,@selector(canBecomeMainWindow),(IMP)canBecomeMain,"c@:");
        Method resize=class_getInstanceMethod(base,@selector(animationResizeTime:));
        class_addMethod(base,@selector(animationResizeTime:),(IMP)resizeDuration,method_getTypeEncoding(resize));
        forceWindow.backgroundColor=NSColor.clearColor;
        forceWindow.opaque=NO;forceWindow.hasShadow=NO;forceWindow.hidesOnDeactivate=NO;forceWindow.ignoresMouseEvents=NO;
        forceWindow.level=NSFloatingWindowLevel;
        forceWindow.collectionBehavior=NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle;
        if (@available(macOS 13.0,*)) forceWindow.collectionBehavior|=NSWindowCollectionBehaviorCanJoinAllApplications;
        forceWindow.movableByWindowBackground=NO;forceWindow.acceptsMouseMovedEvents=YES;
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        placeWindow(NO);[forceWindow makeMainWindow];[forceWindow orderFrontRegardless];
        desktopTarget=[ForceDesktop new];
        forceStatus=[[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        NSImage *image=[NSImage imageWithSystemSymbolName:@"timer" accessibilityDescription:@"Force"];
        image.template=YES;forceStatus.button.image=image;forceStatus.button.toolTip=@"Force · 专注计时";
        forceStatus.button.font=[NSFont monospacedDigitSystemFontOfSize:12 weight:NSFontWeightRegular];
        NSMenu *menu=[NSMenu new];
        NSMenuItem *show=[menu addItemWithTitle:@"显示计时条" action:@selector(show:) keyEquivalent:@""];show.target=desktopTarget;
        NSMenuItem *reset=[menu addItemWithTitle:@"重置计时条位置" action:@selector(reset:) keyEquivalent:@""];reset.target=desktopTarget;
        [menu addItem:NSMenuItem.separatorItem];
        NSMenuItem *quit=[menu addItemWithTitle:@"退出 Force（计时继续）" action:@selector(quit:) keyEquivalent:@""];quit.target=desktopTarget;
        forceStatus.menu=menu;
        globalMouse=[NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown|NSEventMaskRightMouseDown handler:^(NSEvent *event){
            if(isExpanded)ForceNativeEvent(2);
        }];
        localMouse=[NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown|NSEventMaskLeftMouseDragged|NSEventMaskLeftMouseUp|NSEventMaskKeyDown handler:^NSEvent *(NSEvent *event){
            // Move from the original pointer/window pair. This avoids AppKit's
            // modal drag loop on a frameless Wails window and stays stable when
            // the pointer leaves the narrow handle during a drag.
            if(dragging && event.type==NSEventTypeLeftMouseDragged){
                NSPoint point=[event.window convertPointToScreen:event.locationInWindow];
                [forceWindow setFrameOrigin:NSMakePoint(dragOrigin.x+point.x-dragStart.x,dragOrigin.y+point.y-dragStart.y)];
                return nil;
            }
            if(dragging && event.type==NSEventTypeLeftMouseUp){dragging=NO;placeWindow(NO);return nil;}
            if(event.type==NSEventTypeLeftMouseDown && event.window==forceWindow){
                NSPoint point=event.locationInWindow;
                NSPoint local=NSMakePoint(point.x,forceWindow.contentView.bounds.size.height-point.y);
                CGFloat width=forceWindow.frame.size.width;
                NSRect handle=isExpanded ? NSMakeRect(20,14,MIN(190,width-168),27) : NSMakeRect(5,0,23,54);
                if(NSPointInRect(local,handle)){
                    dragging=YES;dragStart=[forceWindow convertPointToScreen:point];dragOrigin=forceWindow.frame.origin;
                    return nil;
                }
            }
            if(isExpanded && ((event.type==NSEventTypeLeftMouseDown && event.window!=forceWindow) || (event.type==NSEventTypeKeyDown && event.window==forceWindow && event.keyCode==53))){
                ForceNativeEvent(2);if(event.type==NSEventTypeKeyDown)return nil;
            }
            return event;
        }];
        moveObserver=[[NSNotificationCenter defaultCenter] addObserverForName:NSWindowDidMoveNotification object:forceWindow queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){
            NSUserDefaults *defaults=NSUserDefaults.standardUserDefaults;
            [defaults setBool:YES forKey:@"force.float.position"];
            [defaults setDouble:NSMaxX(forceWindow.frame) forKey:@"force.float.right"];
            [defaults setDouble:NSMaxY(forceWindow.frame) forKey:@"force.float.top"];
        }];
        screenObserver=[[NSNotificationCenter defaultCenter] addObserverForName:NSApplicationDidChangeScreenParametersNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){placeWindow(NO);ForceNativeEvent(3);}];
        wakeObserver=[NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidWakeNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){placeWindow(NO);ForceNativeEvent(5);}];
        NSData *json=[NSJSONSerialization dataWithJSONObject:geometry() options:0 error:nil];
        result=strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
    });
    return result;
}
char *ForceGeometry(void) {
    __block char *result;
    onMain(^{NSData *json=[NSJSONSerialization dataWithJSONObject:geometry() options:0 error:nil];result=strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);});
    return result;
}
void ForceExpand(int expanded) {
    dispatch_async(dispatch_get_main_queue(),^{
        isExpanded=expanded!=0;
        if(isExpanded){placeWindow(NO);[NSApp activateIgnoringOtherApps:YES];[forceWindow makeKeyAndOrderFront:nil];}
        else { placeWindow(NO);[forceWindow resignKeyWindow]; }
    });
}
void ForceHide(void) { dispatch_async(dispatch_get_main_queue(),^{isExpanded=NO;[forceWindow orderOut:nil];}); }
void ForceUpdateMenu(const char *raw) {
    NSString *time=[NSString stringWithUTF8String:raw];
    dispatch_async(dispatch_get_main_queue(),^{forceStatus.button.title=time.length ? [@" " stringByAppendingString:time] : @"";});
}
void ForceShowDetails(const char *raw) {
    NSString *requested=[NSString stringWithUTF8String:raw];
    dispatch_async(dispatch_get_main_queue(),^{
        detailView=[requested isEqualToString:@"week"] ? @"week" : @"history";
        if(!detailWindow){
            WKWebView *primary=findWebView(forceWindow.contentView);
            WKWebViewConfiguration *config=[WKWebViewConfiguration new];
            [config setURLSchemeHandler:[primary.configuration urlSchemeHandlerForURLScheme:@"wails"] forURLScheme:@"wails"];
            config.suppressesIncrementalRendering=NO;
            config.preferences.tabFocusesLinks=YES;
            [config.userContentController addScriptMessageHandler:desktopTarget name:@"forceDetails"];
            // Wails' asset server injects its runtime. Ignore its messages in this
            // secondary view; the dedicated API below handles records requests.
            [config.userContentController addScriptMessageHandler:desktopTarget name:@"external"];
            NSString *bridge=@"(()=>{let serial=0;const pending=new Map();const call=(method,args=[])=>new Promise((resolve,reject)=>{const id=Date.now()+'-'+(++serial);const timer=setTimeout(()=>{pending.delete(id);reject(new Error('读取超时，请重试'));},15000);pending.set(id,{resolve,reject,timer});window.webkit.messageHandlers.forceDetails.postMessage(JSON.stringify({id,method,args}));});window.forceDetailsResolve=data=>{const item=pending.get(data.id);if(!item)return;clearTimeout(item.timer);pending.delete(data.id);data.error?item.reject(new Error(data.error)):item.resolve(data.result);};window.forceDetailsBackend={ReadState:()=>call('ReadState'),ReadHistory:(start,end)=>call('ReadHistory',[start,end]),ReadWeek:()=>call('ReadWeek')};})();";
            [config.userContentController addUserScript:[[WKUserScript alloc] initWithSource:bridge injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
            detailWindow=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,660,620) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
            detailWindow.title=@"Force · 时间记录";detailWindow.minSize=NSMakeSize(600,480);detailWindow.releasedWhenClosed=NO;
            detailWindow.delegate=desktopTarget;detailWindow.appearance=[NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
            detailWindow.backgroundColor=[NSColor colorWithWhite:21.0/255 alpha:1];
            detailWebView=[[WKWebView alloc] initWithFrame:detailWindow.contentView.bounds configuration:config];
            detailWebView.autoresizingMask=NSViewWidthSizable|NSViewHeightSizable;detailWebView.navigationDelegate=desktopTarget;
            [detailWindow.contentView addSubview:detailWebView];[detailWindow center];
            NSString *url=[NSString stringWithFormat:@"wails://wails/?window=details&view=%@",detailView];
            [detailWebView loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:url]]];
        }else notifyDetailView();
        [NSApp activateIgnoringOtherApps:YES];[detailWindow makeKeyAndOrderFront:nil];
    });
}
void ForceDetailsResolve(const char *raw) {
    NSString *json=[NSString stringWithUTF8String:raw];
    dispatch_async(dispatch_get_main_queue(),^{[detailWebView evaluateJavaScript:[@"window.forceDetailsResolve(" stringByAppendingFormat:@"%@)",json] completionHandler:nil];});
}
void ForceShutdown(void) {
    onMain(^{
        if(globalMouse)[NSEvent removeMonitor:globalMouse];if(localMouse)[NSEvent removeMonitor:localMouse];
        if(screenObserver)[NSNotificationCenter.defaultCenter removeObserver:screenObserver];
        if(moveObserver)[NSNotificationCenter.defaultCenter removeObserver:moveObserver];
        if(wakeObserver)[NSWorkspace.sharedWorkspace.notificationCenter removeObserver:wakeObserver];
        [detailWebView.configuration.userContentController removeAllScriptMessageHandlers];
        detailWindow.delegate=nil;[detailWindow close];detailWindow=nil;detailWebView=nil;
        if(forceStatus)[[NSStatusBar systemStatusBar] removeStatusItem:forceStatus];
    });
}

char *ForceInspectWindow(void) {
    __block char *result;
    onMain(^{
        NSRect window=forceWindow.frame, screen=targetScreen().frame;
        NSDictionary *data=@{@"x":@(NSMinX(window)),@"top":@(NSMaxY(screen)-NSMaxY(window)),@"width":@(window.size.width),@"height":@(window.size.height),@"screenWidth":@(screen.size.width),@"screenHeight":@(screen.size.height)};
        NSData *json=[NSJSONSerialization dataWithJSONObject:data options:0 error:nil];
        result=strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
    });
    return result;
}
