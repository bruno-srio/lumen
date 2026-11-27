FROM registry.fedoraproject.org/fedora-toolbox:44

RUN dnf -y install \
    gcc \
    pkgconf \
    rust \
    cargo \
    rustfmt \
    clippy \
    gtk4-devel \
    libadwaita-devel \
    graphene-devel \
    vulkan-loader-devel \
    blueprint-compiler \
    && dnf clean all
