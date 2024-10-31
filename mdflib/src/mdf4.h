#pragma once

#pragma pack(push, 1)

typedef struct
{
  mdf_link_t dgblock;
  mdf_link_t cgblock;
  mdf_link_t cnblock;
} CHANNEL64;

typedef struct
{
  uint32_t id;
  uint32_t reserved;
  uint64_t length;
  uint64_t link_count;
} BLOCK_HEADER;

typedef struct
{
  mdf_link_t link[1];
} BLOCK_LINKS;

typedef struct
{
  uint32_t id;
  uint32_t reserved;
  uint64_t length;
  uint64_t link_count;
  mdf_link_t link[1];
} BLOCK_HEADER_AND_LINKS;

#define MDF4_ID_PREFIX ((uint32_t)('#') + 0x100 * ('#'))
#define GENERATE_ID(A, B) (MDF4_ID_PREFIX + 0x10000 * (A) + 0x1000000 * (B))

#define MDF4_SYNC_NONE 0
#define MDF4_SYNC_TIME 1

#define MDF4_TIME_FLAG_LOCAL_TIME (1 << 0)
#define MDF4_TIME_FLAG_OFFSETS_VALID (1 << 1)

#define MDF4_ID_LENGTH 64

#define MDF4_ID_FILE 8
#define MDF4_ID_VERS 8
#define MDF4_ID_PROG 8

#define MDF4_ID_FILE_STRING "MDF         "
#define MDF4_ID_VERS_STRING "4.20    "
#define MDF4_ID_VERS_NO 420
#define MDF4_ID_PROG_STRING "........"

#define MDF4_ID_VERS_STRING_400 "4.00    "
#define MDF4_ID_VERS_NO_400 400

#define MDF4_ID_VERS_STRING_410 "4.10    "
#define MDF4_ID_VERS_NO_410 410

#define MDF4_ID_VERS_STRING_411 "4.11    "
#define MDF4_ID_VERS_NO_411 411

#define MDF4_ID_VERS_STRING_420 "4.20    "
#define MDF4_ID_VERS_NO_420 420

#define MDF4_ID_UNFINALIZED "UnFinMF "

#define MDF4_ID_UNFIN_FLAG_INVAL_CYCLE_COUNT_CG (1 << 0)
#define MDF4_ID_UNFIN_FLAG_INVAL_CYCLE_COUNT_SR (1 << 1)
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_DT (1 << 2)
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_RD (1 << 3)
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_DL (1 << 4)
#define MDF4_ID_UNFIN_FLAG_INVAL_VLSD_CG_SD_LEN (1 << 5)
#define MDF4_ID_UNFIN_FLAG_INVAL_UNSORTED_VLSD_OFFSET (1 << 6)

#define MDF_ID_UNFIN_FLAG_CUSTOM_INVERSE_CHAIN_DL (1 << 0)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_CANAPE (1 << 1)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_MDF4LIB (1 << 2)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_MDF4LIB_EX (1 << 3)
#define MDF_ID_UNFIN_FLAG_CUSTOM_RING_BUFFER (1 << 4)

typedef struct idblock64
{

  uint8_t id_file[MDF4_ID_FILE];
  uint8_t id_vers[MDF4_ID_VERS];
  uint8_t id_prog[MDF4_ID_PROG];
  uint8_t id_reserved1[4];
  uint16_t id_ver;
  uint8_t id_reserved2[30];
  uint16_t id_unfin_flags;
  uint16_t id_custom_unfin_flags;
} IDBLOCK64;

#define MDF4_HD_TIME_SRC_PC 0
#define MDF4_HD_TIME_SRC_EXTERNAL 10
#define MDF4_HD_TIME_SRC_ABS_SYNC 16

#define MDF4_HD_ID GENERATE_ID('H', 'D')
#define MDF4_HD_MIN_LENGTH (sizeof(BLOCK_HEADER) + sizeof(HDBLOCK_LINKS) + sizeof(HDBLOCK_DATA))
#define MDF4_HD_MIN_LINK_COUNT (sizeof(HDBLOCK_LINKS) / sizeof(mdf_link_t))

#define MDF4_HD_FLAG_ANGLE_VALID (1 << 0)
#define MDF4_HD_FLAG_DISTANCE_VALID (1 << 1)

typedef struct hdblock_links
{
  mdf_link_t hd_dg_first;
  mdf_link_t hd_fh_first;
  mdf_link_t hd_ch_tree;
  mdf_link_t hd_at_first;
  mdf_link_t hd_ev_first;
  mdf_link_t hd_md_comment;

} HDBLOCK_LINKS;

typedef struct hdblock_data
{
  uint64_t hd_start_time_ns;
  int16_t hd_tz_offset_min;

  int16_t hd_dst_offset_min;

  uint8_t hd_time_flags;
  uint8_t hd_time_class;
  uint8_t hd_flags;
  uint8_t hd_reserved;
  double hd_start_angle_rad;
  double hd_start_distance_m;
} HDBLOCK_DATA;

#define MDF4_MD_ID GENERATE_ID('M', 'D')
#define MDF4_MD_MIN_LENGTH (sizeof(BLOCK_HEADER))
#define MDF4_MD_LINK_COUNT (0)

typedef struct mdblock_data
{

  char s[1];
} MDBLOCK_DATA;

#define MDF4_TX_ID GENERATE_ID('T', 'X')
#define MDF4_TX_MIN_LENGTH (sizeof(BLOCK_HEADER))
#define MDF4_TX_LINK_COUNT (0)

typedef struct txblock_data
{

  char s[1];
} TXBLOCK_DATA;

#define MDF4_FH_ID GENERATE_ID('F', 'H')
#define MDF4_FH_MIN_LENGTH (sizeof(BLOCK_HEADER) + sizeof(FHBLOCK_LINKS) + sizeof(FHBLOCK_DATA))
#define MDF4_FH_MIN_LINK_COUNT (sizeof(FHBLOCK_LINKS) / sizeof(mdf_link_t))

typedef struct fhblock_links
{
  mdf_link_t fh_fh_next;
  mdf_link_t fh_md_comment;

} FHBLOCK_LINKS;

typedef struct fhblock_data
{
  uint64_t fh_time_ns;
  int16_t fh_tz_offset_min;

  int16_t fh_dst_offset_min;

  uint8_t fh_time_flags;
  uint8_t fh_reserved[3];

} FHBLOCK_DATA;

#define MDF4_CH_ID GENERATE_ID('C', 'H')
#define MDF4_CH_MIN_LENGTH (sizeof(BLOCK_HEADER) + 5 * 8)
#define MDF4_CH_MIN_LINK_COUNT ((sizeof(HDBLOCK_LINKS) - sizeof(CHANNEL64)) / sizeof(mdf_link_t))

#define MDF4_CH_TYPE_GROUP 0
#define MDF4_CH_TYPE_FUNCTION 1
#define MDF4_CH_TYPE_STRUCTURE 2
#define MDF4_CH_TYPE_MAP_LIST 3

#define MDF4_CH_TYPE_MEAS_INPUT 4
#define MDF4_CH_TYPE_MEAS_OUTPUT 5
#define MDF4_CH_TYPE_MEAS_LOCAL 6
#define MDF4_CH_TYPE_CAL_DEF 7
#define MDF4_CH_TYPE_CAL_REF 8

typedef struct chblock_links
{
  mdf_link_t ch_ch_next;
  mdf_link_t ch_ch_first;
  mdf_link_t ch_tx_name;
  mdf_link_t ch_md_comment;

  CHANNEL64 ch_element[1];
} CHBLOCK_LINKS;

typedef struct chblock_data
{
  uint32_t ch_element_count;
  uint8_t ch_type;
  uint8_t ch_reserved[3];
} CHBLOCK_DATA;


#define MDF4_DG_ID GENERATE_ID('D', 'G')
#define MDF4_DG_MIN_LENGTH (sizeof(BLOCK_HEADER) + 5 * 8)
#define MDF4_DG_MIN_LINK_COUNT (sizeof(DGBLOCK_LINKS) / sizeof(mdf_link_t))

typedef struct dgblock_links
{
  mdf_link_t dg_dg_next;
  mdf_link_t dg_cg_first;
  mdf_link_t dg_data;
  mdf_link_t dg_md_comment;

} DGBLOCK_LINKS;

typedef struct dgblock_data
{
  uint8_t dg_rec_id_size;
  uint8_t dg_reserved[7];
} DGBLOCK_DATA;

#define MDF4_CG_ID GENERATE_ID('C', 'G')
#define MDF4_CG_MIN_LENGTH (sizeof(BLOCK_HEADER) + 10 * 8)
#define MDF4_CG_MIN_LINK_COUNT (sizeof(CGBLOCK_LINKS) / sizeof(mdf_link_t))

#define MDF4_CG_FLAG_VLSD (1 << 0)
#define MDF4_CG_FLAG_BUS_EVENT (1 << 1)
#define MDF4_CG_FLAG_PLAIN_BUS_EVENT (1 << 2)
#define MDF4_CG_FLAG_REMOTE_MASTER (1 << 3)
#define MDF4_CG_FLAG_EVENT_SIGNAL (1 << 4)

typedef struct cgblock_links
{
  mdf_link_t cg_cg_next;
  mdf_link_t cg_cn_first;
  mdf_link_t cg_tx_acq_name;
  mdf_link_t cg_si_acq_source;
  mdf_link_t cg_sr_first;
  mdf_link_t cg_md_comment;

} CGBLOCK_LINKS;

typedef struct cgblock_data
{
  uint64_t cg_record_id;
  uint64_t cg_cycle_count;
  uint16_t cg_flags;
  uint16_t cg_path_separator;
  uint8_t cg_reserved[4];
  union
  {
    uint64_t cg_sdblock_length;
    struct
    {
      uint32_t cg_data_bytes;
      uint32_t cg_inval_bytes;
    } cg_record_bytes;
  };

} CGBLOCK_DATA;


#define MDF4_CN_ID GENERATE_ID('C', 'N')
#define MDF4_CN_MIN_LENGTH (sizeof(BLOCK_HEADER) + 17 * 8)
#define MDF4_CN_MIN_LINK_COUNT (sizeof(CNBLOCK_LINKS) / sizeof(mdf_link_t))

#define MDF4_CN_TYPE_VALUE 0
#define MDF4_CN_TYPE_VLSD 1
#define MDF4_CN_TYPE_MASTER 2
#define MDF4_CN_TYPE_VIRTUAL_MASTER 3

#define MDF4_CN_TYPE_STREAM_SYNC 4
#define MDF4_CN_TYPE_MLSD 5

#define MDF4_CN_TYPE_VIRTUAL_DATA 6

#define MDF4_CN_SYNC_NONE MDF4_SYNC_NONE
#define MDF4_CN_SYNC_TIME MDF4_SYNC_TIME
#define MDF4_CN_SYNC_ANGLE MDF4_SYNC_ANGLE
#define MDF4_CN_SYNC_DISTANCE MDF4_SYNC_DISTANCE
#define MDF4_CN_SYNC_INDEX MDF4_SYNC_INDEX

#define MDF4_CN_VAL_UNSIGN_INTEL 0
#define MDF4_CN_VAL_UNSIGN_MOTOROLA 1
#define MDF4_CN_VAL_SIGNED_INTEL 2
#define MDF4_CN_VAL_SIGNED_MOTOROLA 3
#define MDF4_CN_VAL_REAL_INTEL 4
#define MDF4_CN_VAL_REAL_MOTOROLA 5

#define MDF4_CN_VAL_STRING_SBC 6
#define MDF4_CN_VAL_STRING_UTF8 7
#define MDF4_CN_VAL_STRING_UTF16_LE 8
#define MDF4_CN_VAL_STRING_UTF16_BE 9
#define MDF4_CN_VAL_BYTE_ARRAY 10
#define MDF4_CN_VAL_MIME_SAMPLE 11
#define MDF4_CN_VAL_MIME_STREAM 12
#define MDF4_CN_VAL_CO_DATE_STRUCT 13

#define MDF4_CN_VAL_CO_TIME_STRUCT 14

#define MDF4_CN_VAL_COMPLEX_INTEL 15

#define MDF4_CN_VAL_COMPLEX_MOTOROLA 16

struct cn_byte_array_date
{
  uint16_t ms : 16;

  uint8_t min : 6;
  uint8_t min_reserved : 2;

  uint8_t hour : 5;
  uint8_t hour_reserved : 2;
  uint8_t summer_time : 1;

  uint8_t day : 5;
  uint8_t week_day : 3;

  uint8_t month : 6;
  uint8_t month_reserved : 2;

  uint8_t year : 7;
  uint8_t year_reserved : 1;
};

struct cn_byte_array_time
{
  uint32_t ms : 28;
  uint32_t ms_reserved : 4;

  uint16_t days : 16;
};

#define MDF4_CN_FLAG_ALL_INVALID (1 << 0)
#define MDF4_CN_FLAG_INVAL_BIT (1 << 1)
#define MDF4_CN_FLAG_PRECISION (1 << 2)
#define MDF4_CN_FLAG_VAL_RANGE_OK (1 << 3)
#define MDF4_CN_FLAG_VAL_LIMIT_OK (1 << 4)
#define MDF4_CN_FLAG_VAL_LIMIT_EXT_OK (1 << 5)
#define MDF4_CN_FLAG_DISCRETE_VALUES (1 << 6)
#define MDF4_CN_FLAG_CALIBRATION (1 << 7)
#define MDF4_CN_FLAG_CALCULATED (1 << 8)
#define MDF4_CN_FLAG_VIRTUAL (1 << 9)
#define MDF4_CN_FLAG_BUS_EVENT (1 << 10)
#define MDF4_CN_FLAG_MONOTONOUS (1 << 11)
#define MDF4_CN_FLAG_DEFAULT_X (1 << 12)
#define MDF4_CN_FLAG_EVENT_SIGNAL (1 << 13)
#define MDF4_CN_FLAG_VLSD_DATA_STREAM (1 << 14)

typedef struct cnblock_links
{
  mdf_link_t cn_cn_next;
  mdf_link_t cn_composition;
  mdf_link_t cn_tx_name;
  mdf_link_t cn_si_source;
  mdf_link_t cn_cc_conversion;
  mdf_link_t cn_data;

  mdf_link_t cn_md_unit;

  mdf_link_t cn_md_comment;

} CNBLOCK_LINKS;

typedef struct cnblock_data
{
  uint8_t cn_type;
  uint8_t cn_sync_type;
  uint8_t cn_data_type;
  uint8_t cn_bit_offset;
  uint32_t cn_byte_offset;
  uint32_t cn_bit_count;

  uint32_t cn_flags;
  uint32_t cn_inval_bit_pos;
  uint8_t cn_precision;

  uint8_t cn_reserved;
  uint16_t cn_attachment_count;
  double cn_val_range_min;
  double cn_val_range_max;
  double cn_limit_min;
  double cn_limit_max;
  double cn_limit_ext_min;
  double cn_limit_ext_max;

} CNBLOCK_DATA;

#define MDF4_CC_ID GENERATE_ID('C', 'C')
#define MDF4_CC_MIN_LENGTH (sizeof(BLOCK_HEADER) + 7 * 8)
#define MDF4_CC_MIN_LINK_COUNT 4

#define MDF4_CC_LENGTH(link_count, para_count) (MDF4_CC_MIN_LENGTH + (link_count) * sizeof(mdf_link_t) + (para_count) * sizeof(double))

#define MDF4_CC_LENGTH_NON MDF4_CC_LENGTH(0, 0)
#define MDF4_CC_LENGTH_LIN MDF4_CC_LENGTH(0, 2)
#define MDF4_CC_LENGTH_RAT MDF4_CC_LENGTH(0, 6)
#define MDF4_CC_LENGTH_ALG MDF4_CC_LENGTH(1, 0)
#define MDF4_CC_LENGTH_TABI(n) MDF4_CC_LENGTH(0, (n) * 2)
#define MDF4_CC_LENGTH_TAB(n) MDF4_CC_LENGTH(0, (n) * 2)
#define MDF4_CC_LENGTH_RTAB(n) MDF4_CC_LENGTH(0, (n) * 3 + 1)
#define MDF4_CC_LENGTH_TABX(n) MDF4_CC_LENGTH((n) + 1, (n))
#define MDF4_CC_LENGTH_RTABX(n) MDF4_CC_LENGTH((n) + 1, (n) * 2)
#define MDF4_CC_LENGTH_TTAB(n) MDF4_CC_LENGTH((n), (n) + 1)
#define MDF4_CC_LENGTH_TRANS(n) MDF4_CC_LENGTH((n) * 2 + 1, 0)
#define MDF4_CC_LENGTH_BFIELD(n) MDF4_CC_LENGTH(n, n)

#define MDF4_CC_LINK_COUNT_NON (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_LIN (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_RAT (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_ALG (MDF4_CC_MIN_LINK_COUNT + 1)
#define MDF4_CC_LINK_COUNT_TABI(n) (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_TAB(n) (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_RTAB(n) (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_TABX(n) (MDF4_CC_MIN_LINK_COUNT + (n) + 1)
#define MDF4_CC_LINK_COUNT_RTABX(n) (MDF4_CC_MIN_LINK_COUNT + (n) + 1)
#define MDF4_CC_LINK_COUNT_TTAB(n) (MDF4_CC_MIN_LINK_COUNT + n)
#define MDF4_CC_LINK_COUNT_TRANS(n) (MDF4_CC_MIN_LINK_COUNT + (n) * 2 + 1)
#define MDF4_CC_LINK_COUNT_BFIELD(n) (MDF4_CC_MIN_LINK_COUNT + n)

#define MDF4_CC_FRM_NON 0

#define MDF4_CC_FRM_LIN 1

#define MDF4_CC_FRM_RAT 2

#define MDF4_CC_FRM_ALG 3

#define MDF4_CC_FRM_TABI 4

#define MDF4_CC_FRM_TAB 5

#define MDF4_CC_FRM_RTAB 6

#define MDF4_CC_FRM_TABX 7

#define MDF4_CC_FRM_RTABX 8

#define MDF4_CC_FRM_TTAB 9

#define MDF4_CC_FRM_TRANS 10

#define MDF4_CC_FRM_BITFIELD_TAB 11

#define MDF4_CC_FLAG_PRECISION (1 << 0)
#define MDF4_CC_FLAG_PHY_RANGE_OK (1 << 1)
#define MDF4_CC_FLAG_STATUS_STRING (1 << 2)

typedef struct ccblock_links
{
  mdf_link_t cc_tx_name;
  mdf_link_t cc_md_unit;

  mdf_link_t cc_md_comment;

  mdf_link_t cc_cc_inverse;

} CCBLOCK_LINKS;

typedef struct ccblock_data
{
  uint8_t cc_type;
  uint8_t cc_precision;
  uint16_t cc_flags;
  uint16_t cc_ref_count;
  uint16_t cc_val_count;
  double cc_phy_range_min;
  double cc_phy_range_max;

  double cc_val[2];

} CCBLOCK_DATA;

#define MDF4_CA_ID GENERATE_ID('C', 'A')
#define MDF4_CA_MIN_LENGTH (sizeof(BLOCK_HEADER) + (3 * 8))
#define MDF4_CA_MIN_LINK_COUNT (sizeof(CABLOCK_LINKS) / sizeof(mdf_link_t))

#define MDF4_CA_STORAGE_CN_TEMPLATE 0

#define MDF4_CA_STORAGE_CG_TEMPLATE 1

#define MDF4_CA_STORAGE_DG_TEMPLATE 2

#define MDF4_CA_TYPE_VAL_ARRAY 0
#define MDF4_CA_TYPE_SCALE_AXIS 1
#define MDF4_CA_TYPE_LOOKUP 2
#define MDF4_CA_TYPE_INTERVAL_AXIS 3
#define MDF4_CA_TYPE_CLASSIFICATION_RESULT 4

#define MDF4_CA_FLAG_DYNAMIC_SIZE (1 << 0)
#define MDF4_CA_FLAG_INPUT_QUANTITY (1 << 1)
#define MDF4_CA_FLAG_OUTPUT_QUANTITY (1 << 2)
#define MDF4_CA_FLAG_COMPARISON_QUANTITY (1 << 3)
#define MDF4_CA_FLAG_AXIS (1 << 4)
#define MDF4_CA_FLAG_FIXED_AXIS (1 << 5)
#define MDF4_CA_FLAG_INVERSE_LAYOUT (1 << 6)

#define MDF4_CA_FLAG_INTERVAL_LEFT_OPEN (1 << 7)

#define MDF4_CA_FLAG_STANDARD_AXIS (1 << 8)

typedef struct cablock_links
{
  mdf_link_t ca_composition;

} CABLOCK_LINKS;

typedef struct cablock_data
{
  uint8_t ca_type;
  uint8_t ca_storage;
  uint16_t ca_ndim;

  uint32_t ca_flags;
  int32_t ca_byte_offset_base;
  uint32_t ca_inval_bit_pos_base;

} CABLOCK_DATA;

#define MDF4_DT_ID GENERATE_ID('D', 'T')
#define MDF4_DT_MIN_LENGTH (sizeof(BLOCK_HEADER))
#define MDF4_DT_LINK_COUNT 0

typedef struct dtblock_data
{
  uint8_t dt_data[1];
} DTBLOCK_DATA;


#pragma pack(pop)
