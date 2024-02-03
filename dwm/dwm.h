#include <X11/Xlib.h>

void run(void);
void checkotherwm(void);
void setup(void);
void scan(void);
void cleanup(void);

long getstate(Window w);
void manage(Window w, XWindowAttributes *wa);


enum { WMProtocols, WMDelete, WMState, WMTakeFocus, WMLast }; /* default atoms */
enum { NetSupported, NetWMName, NetWMState, NetWMCheck,
       NetWMFullscreen, NetActiveWindow, NetWMWindowType,
       NetWMWindowTypeDialog, NetClientList, NetLast }; /* EWMH atoms */
Atom wmatom[WMLast], netatom[NetLast];

Display *dpy;
int running = 1;
Window root;
