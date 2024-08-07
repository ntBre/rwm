find . -name '*.rs' -exec grep -EnH 'bindgen::[a-z_]+\(' {} \;
