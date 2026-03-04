# Documentation book (mdbook)

# Build the RACFS Book (output in docs/book/build/)
book:
    cd '{{ justfile_directory() }}/docs/book' && mdbook build

# Serve the book locally with live reload (http://localhost:3000)
book-serve:
    cd '{{ justfile_directory() }}/docs/book' && mdbook serve --open
