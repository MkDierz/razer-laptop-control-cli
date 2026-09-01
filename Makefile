NAME    := razer-cli
VERSION := 0.3.0

PREFIX      ?= /usr/local
BINDIR      ?= $(PREFIX)/bin
MANDIR      ?= $(PREFIX)/share/man/man1
UDEVDIR     ?= $(PREFIX)/lib/udev/rules.d
DESTDIR     ?=

CARGO    := cargo
INSTALL  := install
FPM      := fpm
ARCH     := $(shell uname -m)

ifeq ($(ARCH),x86_64)
  DEB_ARCH := amd64
else ifeq ($(ARCH),aarch64)
  DEB_ARCH := arm64
else
  DEB_ARCH := $(ARCH)
endif

PKGDIR := $(CURDIR)/target/pkg
PKG_PREFIX := /usr
DEB_STAGE := $(PKGDIR)/deb-stage
RPM_STAGE := $(PKGDIR)/rpm-stage
PACMAN_STAGE := $(PKGDIR)/pacman-stage

.PHONY: all build man install uninstall clean help \
        deb rpm pacman pkg-clean

all: build

build:
	$(CARGO) build --release

man: docs/razer-cli.1

install:
	@test -f target/release/$(NAME) || { echo "Error: binary not found. Run 'make build' first." >&2; exit 1; }
	$(INSTALL) -Dm 0755 target/release/$(NAME)  $(DESTDIR)$(BINDIR)/$(NAME)
	$(INSTALL) -Dm 0644 docs/razer-cli.1        $(DESTDIR)$(MANDIR)/$(NAME).1
	$(INSTALL) -Dm 0644 data/udev/99-razer.rules $(DESTDIR)$(UDEVDIR)/99-razer.rules

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/$(NAME)
	rm -f $(DESTDIR)$(MANDIR)/$(NAME).1
	rm -f $(DESTDIR)$(UDEVDIR)/99-razer.rules

clean:
	$(CARGO) clean
	rm -rf $(PKGDIR)

# --- FPM packaging -----------------------------------------------------------

pkg-clean:
	rm -rf $(PKGDIR)

$(DEB_STAGE)/.stamp:
	rm -rf $(DEB_STAGE)
	$(MAKE) install DESTDIR=$(DEB_STAGE) PREFIX=$(PKG_PREFIX)
	@touch $@

$(RPM_STAGE)/.stamp:
	rm -rf $(RPM_STAGE)
	$(MAKE) install DESTDIR=$(RPM_STAGE) PREFIX=$(PKG_PREFIX)
	@touch $@

$(PACMAN_STAGE)/.stamp:
	rm -rf $(PACMAN_STAGE)
	$(MAKE) install DESTDIR=$(PACMAN_STAGE) PREFIX=$(PKG_PREFIX)
	@touch $@

deb: $(DEB_STAGE)/.stamp
	$(FPM) -t deb \
		-p $(PKGDIR)/$(NAME)_$(VERSION)-1_$(DEB_ARCH).deb \
		--architecture $(DEB_ARCH) \
		--depends libudev0 \
		--depends libusb-1.0-0 \
		-C $(DEB_STAGE) \
		.

rpm: $(RPM_STAGE)/.stamp
	$(FPM) -t rpm \
		-p $(PKGDIR)/$(NAME)-$(VERSION)-1.$(ARCH).rpm \
		--architecture $(ARCH) \
		--depends systemd \
		--depends libusb1 \
		-C $(RPM_STAGE) \
		.

pacman: $(PACMAN_STAGE)/.stamp
	$(FPM) -t pacman \
		-p $(PKGDIR)/$(NAME)-$(VERSION)-1-$(ARCH).pkg.tar.zst \
		--architecture $(ARCH) \
		--depends systemd \
		--depends libusb \
		-C $(PACMAN_STAGE) \
		.

# --- End FPM packaging -------------------------------------------------------

help:
	@echo "Targets:"
	@echo "  build    - Build release binary (default)"
	@echo "  man      - Man page (static, always up to date)"
	@echo "  install  - Install binary, man page, and udev rule"
	@echo "  uninstall- Remove installed files"
	@echo "  deb      - Build .deb package (FPM)"
	@echo "  rpm      - Build .rpm package (FPM)"
	@echo "  pacman   - Build .pkg.tar.zst package (FPM)"
	@echo "  pkg-clean- Remove package staging directory"
	@echo "  clean    - Remove build and package artifacts"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=$(PREFIX)  BINDIR=$(BINDIR)"
	@echo "  DESTDIR=$(DESTDIR)  (for staging installs)"
	@echo "  ARCH=$(ARCH)  DEB_ARCH=$(DEB_ARCH)"
