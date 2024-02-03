#include <X11/Xlib.h>

void run(void);
void checkotherwm(void);
void setup(void);
void scan(void);
void cleanup(void);

Display *dpy;
int running = 1;
