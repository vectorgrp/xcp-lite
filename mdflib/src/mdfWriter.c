/*  mdfWriter.c */

#include "main.h"

#define mdf_link_t uint64_t

#ifndef _WIN
#define ftell (mdf_link_t) ftello
#define fseek(f, o, m) fseeko(f, (off_t)o, m)
#else
#define ftell (mdf_link_t) _ftelli64
#define fseek(f, o, m) _fseeki64(f, (__int64)o, m)
#endif

#include "mdf4.h"
#include "mdfWriter.h"

#define MDF_TIME_CHANNEL_SIZE 4
#define MD_COMMENT_LEN 512
#define CC_UNIT_LEN 32
#define CN_NAME_LEN 32

#pragma pack(push, 1)

struct mdfHeaderBlock {
    IDBLOCK64 id;
    BLOCK_HEADER hdHeader;
    HDBLOCK_LINKS hdLinks;
    HDBLOCK_DATA hdData;

    BLOCK_HEADER fhHeader;
    FHBLOCK_LINKS fhLinks;
    FHBLOCK_DATA fhData;

    BLOCK_HEADER mdHeader;
    char mdData[MD_COMMENT_LEN];

    BLOCK_HEADER dgHeader;
    DGBLOCK_LINKS dgLinks;
    DGBLOCK_DATA dgData;
};

struct mdfChannelGroupBlock {
    BLOCK_HEADER cgHeader;
    CGBLOCK_LINKS cgLinks;
    CGBLOCK_DATA cgData;
};

struct mdfChannelBlock {
    BLOCK_HEADER cnHeader;
    CNBLOCK_LINKS cnLinks;
    CNBLOCK_DATA cnData;

    BLOCK_HEADER ccHeader;
    CCBLOCK_LINKS ccLinks;
    CCBLOCK_DATA ccData;

    BLOCK_HEADER txHeaderUnit;
    char unit[CC_UNIT_LEN];

    BLOCK_HEADER txHeader;
    char name[CN_NAME_LEN];
};

struct mdfArrayBlock {
    BLOCK_HEADER cnHeader;
    CNBLOCK_LINKS cnLinks;
    CNBLOCK_DATA cnData;

    BLOCK_HEADER ccHeader;
    CCBLOCK_LINKS ccLinks;
    CCBLOCK_DATA ccData;

    BLOCK_HEADER txHeaderUnit;
    char unit[CC_UNIT_LEN];

    BLOCK_HEADER txHeader;
    char name[CN_NAME_LEN];

    BLOCK_HEADER caHeader;
    CABLOCK_LINKS caLinks;
    CABLOCK_DATA caData;
    uint64_t ca_dim_size[1];
};

#pragma pack(pop)

struct mdfDataBlock {
    BLOCK_HEADER dtHeader;
};

struct mdfChannelGroup {

    struct mdfChannelGroupBlock b;

    uint32_t recordLen;       /* including recordIdLen */
    uint32_t actualRecordLen; /* including recordIdLen */

    struct mdfChannel *timeChannel;
    struct mdfChannel *dataChannelFirst;
    struct mdfChannel *dataChannelLast;
    struct mdfChannelGroup *next;

    uint32_t recordId;
    uint32_t dataChannelCount;

    uint32_t groupHeaderSize;
    mdf_link_t pos;
};

struct mdfChannel {

    union {
        struct mdfChannelBlock c;
        struct mdfArrayBlock a;
    } b;

    struct mdfChannel *next;

    uint32_t channelHeaderSize;
    mdf_link_t pos;
};

static FILE *mdfFile = NULL;

static struct mdfHeaderBlock *mdfHeader = NULL;
static uint32_t mdfRecordIdLen = 2;

static struct mdfChannelGroup *mdfChannelGroupFirst = NULL;
static struct mdfChannelGroup *mdfChannelGroupLast = NULL;
static uint32_t mdfChannelGroupCount = 0;

static struct mdfDataBlock *mdfDataBlock = NULL;
static mdf_link_t mdfDataBlockPos;
static uint64_t mdfDataBlockLen = 0;
static uint64_t mdfCycleCount = 0;

// Header
static struct mdfHeaderBlock *mdfCreateHeaderBlock(int unfin, mdf_link_t dataLink, uint32_t recordIdSize) {

    printf("mdfCreateHeaderBlock data=%" PRIu64 "\n", dataLink);

    // _time64(&localTime);
    // localTime *= 1000000000ULL;

    struct mdfHeaderBlock *h = malloc(sizeof(struct mdfHeaderBlock));
    if (h == NULL)
        return NULL;
    memset(h, 0, sizeof(struct mdfHeaderBlock));

    if (unfin) {
        memcpy(h->id.id_file, MDF4_ID_UNFINALIZED, MDF4_ID_FILE); /* 8*/ /* file identification     (always MDF4_ID_FILE,  no \0 termination, always SBC string) */
        h->id.id_unfin_flags = MDF4_ID_UNFIN_FLAG_INVAL_CYCLE_COUNT_CG | MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_DT;
        h->id.id_custom_unfin_flags = 0;
    } else {
        memcpy(h->id.id_file, MDF4_ID_FILE_STRING, MDF4_ID_FILE); /* 8*/ /* file identification     (always MDF4_ID_FILE,  no \0 termination, always SBC string) */
        h->id.id_unfin_flags = 0;
        h->id.id_custom_unfin_flags = 0;
    }
    memcpy(h->id.id_vers, MDF4_ID_VERS_STRING_410, MDF4_ID_VERS); /* 8*/ /* format identification   (MDF4_ID_VERS_STRING,  no \0 termination, always SBC string) */
    memcpy(h->id.id_prog, "XCPsim2 ", MDF4_ID_PROG); /* 8*/              /* program identification  (e.g. "CANape", etc,   no \0 termination, always SBC string) */
    h->id.id_ver = MDF4_ID_VERS_NO_410; /* 2*/                           /* default version number  (MDF4_ID_VERS_NO) */

    h->hdHeader.id = GENERATE_ID('H', 'D');
    h->hdHeader.length = MDF4_HD_MIN_LENGTH;
    h->hdHeader.link_count = MDF4_HD_MIN_LINK_COUNT;
    h->hdLinks.hd_dg_first = MDF4_ID_LENGTH + MDF4_HD_MIN_LENGTH + MDF4_FH_MIN_LENGTH + MDF4_MD_MIN_LENGTH + MD_COMMENT_LEN; /* pointer to (first) data group block (DGBLOCK) */
    h->hdLinks.hd_fh_first = MDF4_ID_LENGTH + MDF4_HD_MIN_LENGTH; /* pointer to (first) file history block      (FHBLOCK) */
    h->hdLinks.hd_ch_tree = 0;                                    /* pointer to (first) channel hierarchy block (CHBLOCK) (can be 0) */
    h->hdLinks.hd_at_first = 0;                                   /* pointer to (first) attachment              (ATBLOCK) (can be 0) */
    h->hdLinks.hd_ev_first = 0;                                   /* pointer to (first) event block             (EVBLOCK) (can be 0) */
    h->hdLinks.hd_md_comment = 0;                                 /* pointer to measurement file comment        (MDBLOCK or TXBLOCK) (can be 0) */
    h->hdData.hd_start_time_ns =
        0; // @@@@ localTime; /*8*/  /* start of measurement in ns elapsed since 00:00 Jan 1, 1970 UTC time or local time (if MDF4_TIME_FLAG_LOCAL_TIME flag set) */
    h->hdData.hd_tz_offset_min = 0;
    /*2*/ /* time zone offset in minutes (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) i.e. local_time_ns = hd_start_time_ns + MDF4_MIN_TO_NS(hd_tz_offset_min) */
    h->hdData.hd_dst_offset_min = 0; /*2*/ /* DST offset for local time in minutes used at start time (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) i.e. local_DST_time_ns =
                                              hd_start_time_ns + MDF4_MIN_TO_NS(hd_tz_offset_min) + MDF4_MIN_TO_NS(hd_dst_offset_min) */
    h->hdData.hd_time_flags = MDF4_TIME_FLAG_LOCAL_TIME; /*1*/ /* time flags are bit combination of [MDF4_TIME_FLAG_xxx] */
    h->hdData.hd_time_class = MDF4_HD_TIME_SRC_PC; /*1*/       /* time quality class [MDF4_HD_TIME_SRC_xxx] */
    h->hdData.hd_flags = 0; /*1*/                              /* flags are bit combination of [MDF4_HD_FLAG_xxx] */

    h->fhHeader.id = GENERATE_ID('F', 'H'); /*4*/          /* identification, Bytes are interpreted as ASCII code */
    h->fhHeader.length = MDF4_FH_MIN_LENGTH; /*8*/         /* total number of Bytes contained in block (block header + link list + block data */
    h->fhHeader.link_count = MDF4_FH_MIN_LINK_COUNT; /*8*/ /* number of elements in link list = number of links following the block header */
    h->fhLinks.fh_md_comment = MDF4_ID_LENGTH + MDF4_HD_MIN_LENGTH + MDF4_FH_MIN_LENGTH;
    h->fhData.fh_time_ns = 0; // localTime @@@@;           /*8*/  /* Time stamp at which the file has been changed/created in ns elapsed since 00:00 Jan 1, 1970 UTC time or local
                              // time (if MDF4_TIME_FLAG_LOCAL_TIME flag set) */
    h->fhData.fh_tz_offset_min = 0;
    /*2*/ /* time zone offset in minutes (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) i.e. local_time_ns = fh_change_time_ns + MDF4_MIN_TO_NS(fh_tz_offset_min) */
    h->fhData.fh_dst_offset_min = 0; /*2*/ /* DST offset for local time in minutes used at start time (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) i.e. local_DST_time_ns =
                                              fh_change_time_ns + MDF4_MIN_TO_NS(fh_tz_offset_min) + MDF4_MIN_TO_NS(fh_dst_offset_min) */
    h->fhData.fh_time_flags = MDF4_TIME_FLAG_LOCAL_TIME; /*1*/ /* time flags are bit combination of [MDF4_TIME_FLAG_xxx] */

    h->mdHeader.id = GENERATE_ID('M', 'D'); /*4*/                   /* identification, Bytes are interpreted as ASCII code */
    h->mdHeader.length = MDF4_MD_MIN_LENGTH + MD_COMMENT_LEN; /*8*/ /* total number of Bytes contained in block (block header + link list + block data */
    h->mdHeader.link_count = 0; /*8*/                               /* number of elements in link list = number of links following the block header */
    strncpy(h->mdData,
            "<FHcomment> <TX>XCPsim2 Test</TX>"
            "<tool_id>XCPsim2</tool_id> <tool_vendor>Vector Informatik GmbH</tool_vendor> <tool_version>1.0</tool_version><user_name>visza</user_name>"
            "<common_properties> <e name = \"author\">visza</e> <e name = \"project\">xcp-lite</e> </common_properties> </FHcomment>\r\n",
            MD_COMMENT_LEN);

    h->dgHeader.id = GENERATE_ID('D', 'G'); /*4*/          /* identification, Bytes are interpreted as ASCII code */
    h->dgHeader.length = MDF4_DG_MIN_LENGTH; /*8*/         /* total number of Bytes contained in block (block header + link list + block data */
    h->dgHeader.link_count = MDF4_DG_MIN_LINK_COUNT; /*8*/ /* number of elements in link list = number of links following the block header */
    h->dgLinks.dg_dg_next = 0;                             /* pointer to (next)  data group block    (DGBLOCK) (can be 0) */
    h->dgLinks.dg_cg_first =
        MDF4_ID_LENGTH + MDF4_HD_MIN_LENGTH + MDF4_FH_MIN_LENGTH + MDF4_MD_MIN_LENGTH + MD_COMMENT_LEN + MDF4_DG_MIN_LENGTH; /* pointer to (first) channel group block (CGBLOCK) */
    h->dgLinks.dg_data = dataLink; /* pointer to         data list block (DLBLOCK/HZBLOCK/LDBLOCK) or a data block (DTBLOCK, DVBLOCK or respective DZBLOCK) (can be 0) */
    h->dgLinks.dg_md_comment = 0;  /* pointer to         TXBLOCK/MDBLOCK with additional comments (can be 0) */
    h->dgData.dg_rec_id_size = recordIdSize; /*1*/ /* Number of Bytes used for record ID [0,1,2,4,8] (at start of record) */

    return h;
}

// Channelgroup
static struct mdfChannelGroup *mdfCreateChannelGroupBlock(uint64_t recordCount, uint16_t recordId, uint32_t recordLen, mdf_link_t channelLink) {

    printf("mdfCreateChannelGroupBlock recordCount=%" PRIu64 " id=%u len=%u firstChannel=%" PRIu64 "\n", recordCount, recordId, recordLen, channelLink);

    struct mdfChannelGroup *h = malloc(sizeof(struct mdfChannelGroup));
    if (h == NULL)
        return NULL;
    memset(h, 0, sizeof(struct mdfChannelGroup));

    h->b.cgHeader.id = GENERATE_ID('C', 'G');          /* identification, Bytes are interpreted as ASCII code */
    h->b.cgHeader.length = MDF4_CG_MIN_LENGTH;         /* total number of Bytes contained in block (block header + link list + block data */
    h->b.cgHeader.link_count = MDF4_CG_MIN_LINK_COUNT; /* number of elements in link list = number of links following the block header */
    h->b.cgLinks.cg_cg_next = 0;
    ;                                       /* pointer to next channel group block (CGBLOCK)       (can be 0)  */
    h->b.cgLinks.cg_cn_first = channelLink; /* pointer to first channel block (CNBLOCK)            (            must be 0 if MDF4_CG_FLAG_VLSD flag is set) */
    h->b.cgLinks.cg_tx_acq_name = 0;
    ; /* pointer to TXBLOCK with acquisition name            (can be 0, must be 0 if MDF4_CG_FLAG_VLSD flag is set) */
    h->b.cgLinks.cg_si_acq_source = 0;
    ; /* pointer to SIBLOCK with acquisition source info     (can be 0, must be 0 if MDF4_CG_FLAG_VLSD flag is set) */
    h->b.cgLinks.cg_sr_first = 0;
    ; /* pointer to first sample reduction block (SRBLOCK)   (can be 0, must be 0 if MDF4_CG_FLAG_VLSD flag is set) */
    h->b.cgLinks.cg_md_comment = 0;
    ; /* pointer to TXBLOCK/MDBLOCK with additional comments (can be 0, must be 0 if MDF4_CG_FLAG_VLSD flag is set) XML schema for contents of MDBLOCK see cg_comment.xsd */
    h->b.cgData.cg_record_id = recordId;                   /* Record identification, can be up to 8 Bytes long */
    h->b.cgData.cg_cycle_count = recordCount;              /* Number of cycles, i.e. number of samples for this channel group */
    h->b.cgData.cg_flags = 0;                              /* Flags are bit combination of [MDF4_CG_FLAG_xxx] */
    h->b.cgData.cg_path_separator = 0;                     /* Value of character as UTF16-LE to be used as path separator, 0 if no path separator specified. (since MDF 4.1) */
    h->b.cgData.cg_record_bytes.cg_data_bytes = recordLen; /* Length of data range in record in Bytes without id */
    h->b.cgData.cg_record_bytes.cg_inval_bytes = 0;        /* Length of invalidation range in record in Bytes */

    return h;
}

// Channel
static struct mdfChannel *mdfCreateChannelBlock(int timeChannel, const char *name, int type, uint32_t dim, uint32_t byteOffset, uint32_t bitCount, mdf_link_t next, double factor,
                                                double offset, const char *unit) {

    /* type = MDF4_CN_VAL_UNSIGN_INTEL, MDF4_CN_VAL_UNSIGN_INTEL, MDF4_CN_VAL_SIGNED_INTEL, MDF4_CN_VAL_REAL_INTEL */

    printf("mdfCreateChannelBlock type=%u next=%" PRIu64 "\n", type, next);

    struct mdfChannel *c = malloc(sizeof(struct mdfChannel));
    if (c == NULL)
        return NULL;
    memset(c, 0, sizeof(struct mdfChannel));

    c->b.c.cnHeader.id = GENERATE_ID('C', 'N');
    c->b.c.cnHeader.length = MDF4_CN_MIN_LENGTH;
    c->b.c.cnHeader.link_count = MDF4_CN_MIN_LINK_COUNT;
    c->b.c.cnLinks.cn_cn_next = next; /* pointer to next CNBLOCK of group (can be 0) */
    if (dim > 1) {
        c->b.c.cnLinks.cn_composition = (char *)&c->b.a.caHeader - (char *)c; /* pointer to CABLOCK/CNBLOCK to describe components of this signal (can be 0) */
    } else {
        c->b.c.cnLinks.cn_composition = 0;
    }
    c->b.c.cnLinks.cn_tx_name = (char *)&c->b.c.txHeader - (char *)c;       /* relative pointer to TXBLOCK for name of this signal (can be 0, must not be 0 for data channels) */
    c->b.c.cnLinks.cn_si_source = 0;                                        /* pointer to SIBLOCK for name of this signal (can be 0, must not be 0 for data channels) */
    c->b.c.cnLinks.cn_cc_conversion = (char *)&c->b.c.ccHeader - (char *)c; // relative pointer to conversion rule (CCBLOCK) of this signal (can be 0) */
    c->b.c.cnLinks.cn_data = 0;       /* pointer to DLBLOCK/HLBLOCK/SDBLOCK/DZBLOCK/ATBLOCK/CGBLOCK defining the signal data for this signal (can be 0) */
    c->b.c.cnLinks.cn_md_unit = 0;    /* pointer to MD/TXBLOCK with string for physical unit after conversion (can be 0). If 0, the unit from the conversion rule applies. If empty
                                         string, no unit should be displayed. */
    c->b.c.cnLinks.cn_md_comment = 0; /* pointer to TXBLOCK/MDBLOCK with comment/description of this signal and other information (can be 0) */
    c->b.c.cnData.cn_type = timeChannel ? MDF4_CN_TYPE_MASTER : MDF4_CN_TYPE_VALUE; /* channel type [MDF4_CN_TYPE_xxx] */
    c->b.c.cnData.cn_sync_type = timeChannel ? MDF4_SYNC_TIME : MDF4_SYNC_NONE;
    c->b.c.cnData.cn_data_type = type;
    c->b.c.cnData.cn_bit_offset = 0;           /* data bit offset for first bit of signal value [0...7]        */
    c->b.c.cnData.cn_byte_offset = byteOffset; /* data byte offset for first bit of signal value               */
    c->b.c.cnData.cn_bit_count = bitCount;     /* Number of bits for the value in record (also relevant for MDF4_CN_VAL_FLOAT / MDF4_CN_VAL_DOUBLE!)   */
    c->b.c.cnData.cn_flags = 0;                /* flags are bit combination of [MDF4_CN_FLAG_xxx] */
    c->b.c.cnData.cn_inval_bit_pos = 0;        /* position of invalidation bit (starting after cg_data_bytes!)    */
    c->b.c.cnData.cn_precision = 0xff; /* precision of value to use for display (for floating-point values) decimal places to use for display of float value (0xFF => infinite) */
    c->b.c.cnData.cn_attachment_count = 0; /* number of attachments related to this channel, i.e. size of cn_at_references. Can be 0. (since MDF 4.1) */
    c->b.c.cnData.cn_val_range_min = 0;    /* minimum value of raw value range */
    c->b.c.cnData.cn_val_range_max = 0;    /* maximum value of raw value range */
    c->b.c.cnData.cn_limit_min = 0;        /* minimum phys value of limit range */
    c->b.c.cnData.cn_limit_max = 0;        /* maximum phys value of limit range */
    c->b.c.cnData.cn_limit_ext_min = 0;    /* minimum phys value of extended limit range */
    c->b.c.cnData.cn_limit_ext_max = 0;    /* maximum phys value of extended limit range */

    c->b.c.ccHeader.id = GENERATE_ID('C', 'C');
    c->b.c.ccHeader.length = MDF4_CC_LENGTH_LIN;
    c->b.c.ccHeader.link_count = MDF4_CC_MIN_LINK_COUNT;
    c->b.c.ccLinks.cc_md_unit = (char *)&c->b.c.txHeaderUnit - (char *)c; /* relative pointer to TX/MDBLOCK with string for physical unit after conversion (can be 0) */
    c->b.c.ccData.cc_type = MDF4_CC_FRM_LIN; /* 1*/                       /* conversion type identifier [MDF4_CC_FRM_xxx] */
    c->b.c.ccData.cc_precision = 0xFF; /* 1*/  /* number of decimal places to use for display of float value (0xFF => infinite) only valid if MDF4_CC_FLAG_PRECISION flag is set */
    c->b.c.ccData.cc_flags = 0; /* 2*/         /* flags are bit combination of [MDF4_CC_FLAG_xxx] */
    c->b.c.ccData.cc_ref_count = 0; /* 2*/     /* length of cc_ref list */
    c->b.c.ccData.cc_val_count = 2; /* 2*/     /* length of cc_val list */
    c->b.c.ccData.cc_phy_range_min = 0; /* 8*/ /* minimum value of physical value range */
    c->b.c.ccData.cc_phy_range_max = 0; /* 8*/ /* maximum value of physical value range */
    c->b.c.ccData.cc_val[0] = offset;
    c->b.c.ccData.cc_val[1] = factor;

    c->b.c.txHeaderUnit.id = GENERATE_ID('T', 'X');
    c->b.c.txHeaderUnit.length = MDF4_TX_MIN_LENGTH + CC_UNIT_LEN;
    c->b.c.txHeaderUnit.link_count = 0;
    strncpy(c->b.c.unit, unit, CC_UNIT_LEN);

    c->b.c.txHeader.id = GENERATE_ID('T', 'X');
    c->b.c.txHeader.length = MDF4_TX_MIN_LENGTH + CN_NAME_LEN;
    c->b.c.txHeader.link_count = 0;
    strncpy(c->b.c.name, name, CN_NAME_LEN);

    c->b.a.caHeader.id = GENERATE_ID('C', 'A');
    c->b.a.caHeader.length = MDF4_CA_MIN_LENGTH + sizeof(c->b.a.ca_dim_size[0]);
    c->b.a.caHeader.link_count = 1;
    c->b.a.caLinks.ca_composition = 0; /* link to CA or CN block for components or link to CA or CN block for list of structure members */
    c->b.a.caData.ca_type = MDF4_CA_TYPE_VAL_ARRAY;
    c->b.a.caData.ca_storage = MDF4_CA_STORAGE_CN_TEMPLATE;
    c->b.a.caData.ca_ndim = 1;
    c->b.a.caData.ca_flags = 0;
    c->b.a.caData.ca_byte_offset_base = bitCount / 8; /* used as base to calculate the Byte offset in case of CN template. Can be negative! */
    c->b.a.caData.ca_inval_bit_pos_base = 0;
    c->b.a.ca_dim_size[0] = dim;

    return c;
}

// Data
static struct mdfDataBlock *mdfCreateDataBlock(void) {

    printf("mdfCreateDataBlock\n");

    struct mdfDataBlock *d = malloc(sizeof(struct mdfDataBlock));
    if (d == NULL)
        return NULL;
    memset(d, 0, sizeof(struct mdfDataBlock));

    d->dtHeader.id = GENERATE_ID('D', 'T');
    d->dtHeader.length = MDF4_DT_MIN_LENGTH;
    d->dtHeader.link_count = 0;
    return d;
}

static void mdfAdjustBlockLinks(BLOCK_HEADER *b, BLOCK_HEADER *root, mdf_link_t offset, mdf_link_t limit) {

    printf(" Adjust %c%c\n", 0xFF & (b->id >> 16), 0xFF & (b->id >> 24));
    BLOCK_LINKS *l = (BLOCK_LINKS *)((char *)b + sizeof(BLOCK_HEADER));
    for (uint32_t i = 0; i < b->link_count; i++) {
        if (l->link[i] != 0) {
            if (l->link[i] < limit) {
                mdfAdjustBlockLinks((BLOCK_HEADER *)((char *)root + l->link[i]), root, offset, limit);
                l->link[i] += offset;
                printf("  %u: %" PRIu64 " -> %" PRIu64 "\n", i, l->link[i] - offset, l->link[i]);
            } else {
                printf("  %u: %" PRIu64 "\n", i, l->link[i]);
            }
        }
    }
}

static int mdfWriteBlock(FILE *f, BLOCK_HEADER *b, uint32_t len, int update) {

    mdf_link_t pos = ftell(f);
    printf("mdfWriteBlock %c%c at %" PRIu64 " len=%u\n", 0xFF & (b->id >> 16), 0xFF & (b->id >> 24), pos, len);
    if (update)
        mdfAdjustBlockLinks(b, b, pos, len);
    return len == fwrite(b, 1, len, f);
}

//------------------------------------------------------------------------------------------------------------------------

int mdfOpen(const char *filename) {

    printf("mdfOpen %s\n", filename);
    if (NULL == (mdfFile = fopen(filename, "wb"))) {
        printf("error: Could not open file %s\n", filename);
        return 0;
    }

    assert(sizeof(long) == 4);

    mdfHeader = NULL;
    mdfChannelGroupFirst = mdfChannelGroupLast = NULL;
    mdfChannelGroupCount = 0;
    mdfRecordIdLen = 2;
    mdfDataBlock = NULL;
    mdfDataBlockPos = 0;
    mdfDataBlockLen = 0;
    mdfCycleCount = 0;

    return 1;
}

int mdfCreateChannelGroup(uint32_t recordId, uint32_t recordLen, uint32_t timeChannelSize, double timeChannelConv) {

    printf("mdfCreateChannelGroup %u\n", recordId);

    uint32_t len = (recordLen <= mdfRecordIdLen) ? 0 : recordLen - mdfRecordIdLen; /*recordLen without id */
    struct mdfChannelGroup *g = mdfCreateChannelGroupBlock(0 /* recordCount*/, recordId /*recordId*/, len /*recordLen without id */, 0 /* channelLink*/);
    if (g == NULL)
        return 0;

    g->pos = 0;
    g->recordId = recordId;
    g->dataChannelFirst = NULL;
    g->dataChannelLast = NULL;
    g->dataChannelCount = 0;

    g->timeChannel = mdfCreateChannelBlock(true, "Time", MDF4_CN_VAL_UNSIGN_INTEL, 1 /* dim*/, 0 /*byteoffset*/, timeChannelSize, 0 /*next*/, timeChannelConv, 0.0, "s");
    if (g->timeChannel == NULL)
        return 0;

    g->recordLen = recordLen;                                    /* including recordIdLen, 0=unknown yet */
    g->actualRecordLen = mdfRecordIdLen + MDF_TIME_CHANNEL_SIZE; /* including recordIdLen */

    if (mdfChannelGroupLast == NULL) {
        mdfChannelGroupFirst = mdfChannelGroupLast = g;
    } else {
        mdfChannelGroupLast->next = g;
        mdfChannelGroupLast = g;
    }
    mdfChannelGroupCount++;
    return 1;
}

int mdfCreateChannel(const char *name, uint8_t msize, int8_t encoding, uint32_t dim, uint32_t byteOffset, double factor, double offset, const char *unit) {

    printf(" mdfCreateChannel %s size=%u signed=%d dim=%u byteOffset=%u\n", name, msize, encoding, dim, byteOffset);

    uint32_t mtype;
    switch (encoding) {
    case 1:
        mtype = MDF4_CN_VAL_UNSIGN_INTEL;
        break;
    case -1:
        mtype = MDF4_CN_VAL_SIGNED_INTEL;
        break;
    case 0:
        mtype = MDF4_CN_VAL_REAL_INTEL;
        break;
    default:
        return 0;
    }

    if (unit == NULL)
        unit = "";
    if (dim < 1)
        dim = 1;
    byteOffset = (byteOffset <= mdfRecordIdLen) ? 0 : byteOffset - mdfRecordIdLen; /*byteOffset without id */

    struct mdfChannelGroup *g = mdfChannelGroupLast;
    if (g == NULL)
        return 0;

    g->actualRecordLen += msize * dim;

    struct mdfChannel *c = mdfCreateChannelBlock(false, name, mtype, dim, byteOffset, msize * 8, 0 /*next*/, factor, offset, unit);
    if (c == NULL)
        return 0;

    if (g->dataChannelLast == NULL) {
        g->dataChannelFirst = g->dataChannelLast = c;
    } else {
        g->dataChannelLast->next = c;
        g->dataChannelLast = c;
    }
    g->dataChannelCount++;
    return 1;
}

int mdfWriteHeader(void) {

    mdf_link_t pos = 0;

    // Eliminate empty channel group groups
    struct mdfChannelGroup *g, **gp;
    for (g = mdfChannelGroupFirst, gp = &mdfChannelGroupFirst; g != NULL; g = g->next) {
        if (g->dataChannelCount == 0) {
            *gp = g->next; // eliminate g
            mdfChannelGroupCount--;
        } else {
            gp = &g->next;
        }
    }

    // Calculate header sizes
    uint64_t headerSize = sizeof(struct mdfHeaderBlock);
    for (struct mdfChannelGroup *g = mdfChannelGroupFirst; g != NULL; g = g->next) {
        assert(g->dataChannelCount != 0);
        g->groupHeaderSize = sizeof(struct mdfChannelGroupBlock) + sizeof(struct mdfChannelBlock);
        for (struct mdfChannel *c = g->dataChannelFirst; c != NULL; c = c->next) {
            if (c->b.c.cnLinks.cn_composition != 0) {
                c->channelHeaderSize = sizeof(struct mdfArrayBlock);
            } else {
                c->channelHeaderSize = sizeof(struct mdfChannelBlock);
            }
            g->groupHeaderSize += c->channelHeaderSize;
        }
        headerSize += g->groupHeaderSize;
    }

    // Header
    mdfHeader = mdfCreateHeaderBlock(true, headerSize /* LINK to data group*/, mdfRecordIdLen);
    if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)mdfHeader, sizeof(struct mdfHeaderBlock), false))
        return 0;
    pos += sizeof(struct mdfHeaderBlock);

    // Channel groups
    for (struct mdfChannelGroup *g = mdfChannelGroupFirst; g != NULL; g = g->next) {

        g->b.cgLinks.cg_cn_first = pos + sizeof(struct mdfChannelGroupBlock);       // First channel
        g->b.cgLinks.cg_cg_next = (g->next == NULL) ? 0 : pos + g->groupHeaderSize; // Next group
        if (g->b.cgData.cg_record_bytes.cg_data_bytes == 0)
            g->b.cgData.cg_record_bytes.cg_data_bytes = g->actualRecordLen - mdfRecordIdLen; /* Length of record in Bytes without id */
        g->pos = pos;
        if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)&g->b, sizeof(struct mdfChannelGroupBlock), false))
            return 0;
        pos += sizeof(struct mdfChannelGroupBlock);

        // Time channel
        struct mdfChannel *tc = g->timeChannel;
        tc->b.c.cnLinks.cn_cn_next = pos + sizeof(struct mdfChannelBlock);
        if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)&tc->b.c, sizeof(struct mdfChannelBlock), true))
            return 0;
        pos += sizeof(struct mdfChannelBlock);

        // Data channels
        for (struct mdfChannel *c = g->dataChannelFirst; c != NULL; c = c->next) {
            if (c->b.c.cnLinks.cn_composition != 0) {

                c->b.c.cnLinks.cn_cn_next = (c->next == NULL) ? 0 : pos + sizeof(struct mdfArrayBlock);
                if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)&c->b.a, sizeof(struct mdfArrayBlock), true))
                    return 0;
                pos += sizeof(struct mdfArrayBlock);
            } else {

                c->b.c.cnLinks.cn_cn_next = (c->next == NULL) ? 0 : pos + sizeof(struct mdfChannelBlock);
                if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)&c->b.c, sizeof(struct mdfChannelBlock), true))
                    return 0;
                pos += sizeof(struct mdfChannelBlock);
            }
        }
    }

    // Data block
    mdfDataBlock = mdfCreateDataBlock();
    mdfDataBlockPos = ftell(mdfFile);
    if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)mdfDataBlock, sizeof(struct mdfDataBlock), false))
        return 0;
    assert(pos == headerSize);
    pos += sizeof(struct mdfDataBlock);

    // Unfinalized: ChannelGroup recordCount and data block size, header unfinalized flags
    return 1;
}

int mdfWriteRecord(const uint8_t *record, uint32_t recordLen) {

    // Increment data group cycle count
    // @@@@@@@@@@@@@@@@
    mdfDataBlockLen += recordLen;
    mdfCycleCount++;
    size_t s = fwrite(record, 1, recordLen, mdfFile);
    return s == recordLen;
}

int mdfClose(void) {

    // uint64_t dataBlockLen = 0;

    if (mdfFile == 0)
        return 1;

    // Finalize
    if (mdfHeader != NULL && mdfDataBlock != NULL && mdfChannelGroupLast != NULL) {

        // Update channel group cycle count
        for (struct mdfChannelGroup *g = mdfChannelGroupFirst; g != NULL; g = g->next) {
            g->b.cgData.cg_cycle_count = mdfCycleCount;
            if (fseek(mdfFile, g->pos, SEEK_SET) != 0)
                return 0;
            if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)g, sizeof(struct mdfChannelGroupBlock), false))
                return 0;
        }

        // Update data block length
        mdfDataBlock->dtHeader.length = MDF4_DT_MIN_LENGTH + mdfDataBlockLen;
        // fsetpos(mdfFile, &mdfDataBlockPos);
        if (fseek(mdfFile, mdfDataBlockPos, SEEK_SET) != 0)
            return 0;
        if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)mdfDataBlock, sizeof(struct mdfDataBlock), false))
            return 0;

        // Update header unfin flags
        memcpy(mdfHeader->id.id_file, MDF4_ID_FILE_STRING, MDF4_ID_FILE);
        mdfHeader->id.id_unfin_flags = 0;
        mdfHeader->id.id_custom_unfin_flags = 0;
        if (fseek(mdfFile, 0, SEEK_SET) != 0)
            return 0;
        if (!mdfWriteBlock(mdfFile, (BLOCK_HEADER *)mdfHeader, sizeof(struct mdfHeaderBlock), false))
            return 0;
    }

    fclose(mdfFile);
    mdfFile = NULL;
    return 1;
}
