#include <X11/Xlib.h>

void run(void);
void checkotherwm(void);
void setup(void);
void scan(void);
void cleanup(void);

long getstate(Window w);
void manage(Window w, XWindowAttributes *wa);

Display *dpy;
int running = 1;
Window root;
