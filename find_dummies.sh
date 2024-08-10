find . -name '*.rs' -exec grep -EnH 'bindgen::[a-z_]+\(' {} \;
find . -name '*.rs' -exec grep -EnH 'bindgen::' {} \;
