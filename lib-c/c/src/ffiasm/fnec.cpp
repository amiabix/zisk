#include "fnec.hpp"
#include <stdio.h>
#include <stdlib.h>
#include <gmp.h>
#include <assert.h>
#include <string>


static mpz_t q;
static mpz_t zero;
static mpz_t one;
static mpz_t mask;
static size_t nBits;
static bool initialized = false;


void Fnec_toMpz(mpz_t r, PFnecElement pE) {
    FnecElement tmp;
    Fnec_toNormal(&tmp, pE);
    if (!(tmp.type & Fnec_LONG)) {
        mpz_set_si(r, tmp.shortVal);
        if (tmp.shortVal<0) {
            mpz_add(r, r, q);
        }
    } else {
        mpz_import(r, Fnec_N64, -1, 8, -1, 0, (const void *)tmp.longVal);
    }
}

void Fnec_fromMpz(PFnecElement pE, mpz_t v) {
    if (mpz_fits_sint_p(v)) {
        pE->type = Fnec_SHORT;
        pE->shortVal = mpz_get_si(v);
    } else {
        pE->type = Fnec_LONG;
        for (int i=0; i<Fnec_N64; i++) pE->longVal[i] = 0;
        mpz_export((void *)(pE->longVal), NULL, -1, 8, -1, 0, v);
    }
}


bool Fnec_init() {
    if (initialized) return false;
    initialized = true;
    mpz_init(q);
    mpz_import(q, Fnec_N64, -1, 8, -1, 0, (const void *)Fnec_q.longVal);
    mpz_init_set_ui(zero, 0);
    mpz_init_set_ui(one, 1);
    nBits = mpz_sizeinbase (q, 2);
    mpz_init(mask);
    mpz_mul_2exp(mask, one, nBits);
    mpz_sub(mask, mask, one);
    return true;
}

void Fnec_str2element(PFnecElement pE, char const *s) {
    mpz_t mr;
    mpz_init_set_str(mr, s, 10);
    mpz_fdiv_r(mr, mr, q);
    Fnec_fromMpz(pE, mr);
    mpz_clear(mr);
}

char *Fnec_element2str(PFnecElement pE) {
    FnecElement tmp;
    mpz_t r;
    if (!(pE->type & Fnec_LONG)) {
        if (pE->shortVal>=0) {
            char *r = new char[32];
            snprintf(r, 32, "%d", pE->shortVal);
            return r;
        } else {
            mpz_init_set_si(r, pE->shortVal);
            mpz_add(r, r, q);
        }
    } else {
        Fnec_toNormal(&tmp, pE);
        mpz_init(r);
        mpz_import(r, Fnec_N64, -1, 8, -1, 0, (const void *)tmp.longVal);
    }
    char *res = mpz_get_str (0, 10, r);
    mpz_clear(r);
    return res;
}

void Fnec_idiv(PFnecElement r, PFnecElement a, PFnecElement b) {
    mpz_t ma;
    mpz_t mb;
    mpz_t mr;
    mpz_init(ma);
    mpz_init(mb);
    mpz_init(mr);

    Fnec_toMpz(ma, a);
    // char *s1 = mpz_get_str (0, 10, ma);
    // printf("s1 %s\n", s1);
    Fnec_toMpz(mb, b);
    // char *s2 = mpz_get_str (0, 10, mb);
    // printf("s2 %s\n", s2);
    mpz_fdiv_q(mr, ma, mb);
    // char *sr = mpz_get_str (0, 10, mr);
    // printf("r %s\n", sr);
    Fnec_fromMpz(r, mr);

    mpz_clear(ma);
    mpz_clear(mb);
    mpz_clear(mr);
}

void Fnec_mod(PFnecElement r, PFnecElement a, PFnecElement b) {
    mpz_t ma;
    mpz_t mb;
    mpz_t mr;
    mpz_init(ma);
    mpz_init(mb);
    mpz_init(mr);

    Fnec_toMpz(ma, a);
    Fnec_toMpz(mb, b);
    mpz_fdiv_r(mr, ma, mb);
    Fnec_fromMpz(r, mr);

    mpz_clear(ma);
    mpz_clear(mb);
    mpz_clear(mr);
}

void Fnec_pow(PFnecElement r, PFnecElement a, PFnecElement b) {
    mpz_t ma;
    mpz_t mb;
    mpz_t mr;
    mpz_init(ma);
    mpz_init(mb);
    mpz_init(mr);

    Fnec_toMpz(ma, a);
    Fnec_toMpz(mb, b);
    mpz_powm(mr, ma, mb, q);
    Fnec_fromMpz(r, mr);

    mpz_clear(ma);
    mpz_clear(mb);
    mpz_clear(mr);
}

void Fnec_inv(PFnecElement r, PFnecElement a) {
    mpz_t ma;
    mpz_t mr;
    mpz_init(ma);
    mpz_init(mr);

    Fnec_toMpz(ma, a);
    mpz_invert(mr, ma, q);
    Fnec_fromMpz(r, mr);
    mpz_clear(ma);
    mpz_clear(mr);
}

void Fnec_div(PFnecElement r, PFnecElement a, PFnecElement b) {
    FnecElement tmp;
    Fnec_inv(&tmp, b);
    Fnec_mul(r, a, &tmp);
}

void Fnec_fail() {
    assert(false);
}


RawFnec::RawFnec() {
    Fnec_init();
    set(fZero, 0);
    set(fOne, 1);
    neg(fNegOne, fOne);
}

RawFnec::~RawFnec() {
}

void RawFnec::fromString(Element &r, const std::string &s, uint32_t radix) {
    mpz_t mr;
    mpz_init_set_str(mr, s.c_str(), radix);
    mpz_fdiv_r(mr, mr, q);
    for (int i=0; i<Fnec_N64; i++) r.v[i] = 0;
    mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, mr);
    Fnec_rawToMontgomery(r.v,r.v);
    mpz_clear(mr);
}

void RawFnec::fromUI(Element &r, unsigned long int v) {
    mpz_t mr;
    mpz_init(mr);
    mpz_set_ui(mr, v);
    for (int i=0; i<Fnec_N64; i++) r.v[i] = 0;
    mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, mr);
    Fnec_rawToMontgomery(r.v,r.v);
    mpz_clear(mr);
}

RawFnec::Element RawFnec::set(int value) {
  Element r;
  set(r, value);
  return r;
}

void RawFnec::set(Element &r, int value) {
  mpz_t mr;
  mpz_init(mr);
  mpz_set_si(mr, value);
  if (value < 0) {
      mpz_add(mr, mr, q);
  }

  mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, mr);
      
  for (int i=0; i<Fnec_N64; i++) r.v[i] = 0;
  mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, mr);
  Fnec_rawToMontgomery(r.v,r.v);
  mpz_clear(mr);
}

std::string RawFnec::toString(const Element &a, uint32_t radix) {
    Element tmp;
    mpz_t r;
    Fnec_rawFromMontgomery(tmp.v, a.v);
    mpz_init(r);
    mpz_import(r, Fnec_N64, -1, 8, -1, 0, (const void *)(tmp.v));
    char *res = mpz_get_str (0, radix, r);
    mpz_clear(r);
    std::string resS(res);
    free(res);
    return resS;
}

void RawFnec::inv(Element &r, const Element &a) {
    mpz_t mr;
    mpz_init(mr);
    mpz_import(mr, Fnec_N64, -1, 8, -1, 0, (const void *)(a.v));
    mpz_invert(mr, mr, q);


    for (int i=0; i<Fnec_N64; i++) r.v[i] = 0;
    mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, mr);

    Fnec_rawMMul(r.v, r.v,Fnec_rawR3);
    mpz_clear(mr);
}

void RawFnec::div(Element &r, const Element &a, const Element &b) {
    Element tmp;
    inv(tmp, b);
    mul(r, a, tmp);
}

#define BIT_IS_SET(s, p) (s[p>>3] & (1 << (p & 0x7)))
void RawFnec::exp(Element &r, const Element &base, uint8_t* scalar, unsigned int scalarSize) {
    bool oneFound = false;
    Element copyBase;
    copy(copyBase, base);
    for (int i=scalarSize*8-1; i>=0; i--) {
        if (!oneFound) {
            if ( !BIT_IS_SET(scalar, i) ) continue;
            copy(r, copyBase);
            oneFound = true;
            continue;
        }
        square(r, r);
        if ( BIT_IS_SET(scalar, i) ) {
            mul(r, r, copyBase);
        }
    }
    if (!oneFound) {
        copy(r, fOne);
    }
}

void RawFnec::toMpz(mpz_t r, const Element &a) {
    Element tmp;
    Fnec_rawFromMontgomery(tmp.v, a.v);
    mpz_import(r, Fnec_N64, -1, 8, -1, 0, (const void *)tmp.v);
}

void RawFnec::fromMpz(Element &r, const mpz_t a) {
    for (int i=0; i<Fnec_N64; i++) r.v[i] = 0;
    mpz_export((void *)(r.v), NULL, -1, 8, -1, 0, a);
    Fnec_rawToMontgomery(r.v, r.v);
}

int RawFnec::toRprBE(const Element &element, uint8_t *data, int bytes)
{
    if (bytes < Fnec_N64 * 8) {
      return -(Fnec_N64 * 8);
    }

    mpz_t r;
    mpz_init(r);
  
    toMpz(r, element);
    
    mpz_export(data, NULL, 1, 8, 1, 0, r);
  
    mpz_clear(r);
    return Fnec_N64 * 8;
}

int RawFnec::fromRprBE(Element &element, const uint8_t *data, int bytes)
{
    if (bytes < Fnec_N64 * 8) {
      return -(Fnec_N64* 8);
    }
    mpz_t r;
    mpz_init(r);

    mpz_import(r, Fnec_N64 * 8, 0, 1, 0, 0, data);
    fromMpz(element, r);

    mpz_clear(r);
    return Fnec_N64 * 8;
}

static bool init = Fnec_init();

RawFnec RawFnec::field;

