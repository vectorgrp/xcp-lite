#pragma once
/* mdf4.h */



#pragma pack(push, 1)



/* reference to a channel */
/* either all 3 links are NIL or
   they reference unambiguously a channel,
   i.e. cgblock must be parent block of cnblock and
   dgblock must be parent block of cgblock */
typedef struct
{
  LINK64  dgblock;     /*8*/    /* data group */
  LINK64  cgblock;     /*8*/    /* channel group */
  LINK64  cnblock;     /*8*/    /* channel */
} CHANNEL64;

/* General definition of block data structure */
/* Each block starts with ID, length and number of links */
typedef struct
{
  DWORD  id;           /*4*/    /* identification, Bytes are interpreted as ASCII code */
  DWORD  reserved;     /*4*/    /* reserved for future use */
  QWORD  length;       /*8*/    /* total number of Bytes contained in block (block header + link list + block data */
  QWORD  link_count;   /*8*/    /* number of elements in link list = number of links following the block header */
} BLOCK_HEADER;        /*total = 24*/

/* variable length link list */
typedef struct
{
  LINK64 link[1];      /* general list of links, can be empty! */
} BLOCK_LINKS;


/* General definition of block data structure */
/* Each block starts with ID, length and number of links */
typedef struct
{
  DWORD  id;           /*4*/    /* identification, Bytes are interpreted as ASCII code */
  DWORD  reserved;     /*4*/    /* reserved for future use */
  QWORD  length;       /*8*/    /* total number of Bytes contained in block (block header + link list + block data */
  QWORD  link_count;   /*8*/    /* number of elements in link list = number of links following the block header */
  LINK64 link[1];      /* general list of links, can be empty! */
} BLOCK_HEADER_AND_LINKS;

// to distinguish from MDF32, all block ids start with ##.
// Using 4 Bytes also allows higher chance for retrieval of lost blocks.
#define MDF4_ID_PREFIX      ((DWORD)('#') + 0x100*('#'))
#define GENERATE_ID(A, B)   ( MDF4_ID_PREFIX + 0x10000*(A) + 0x1000000*(B) )

/* plausibility check for block pointer */
#define isblockMDF4(p) ( ((p) != NULL) \
                   && ((p)->length >= sizeof(BLOCK_HEADER)) \
                   && (((((p)->id) >>  0) & 0xFF) == '#') \
                   && (((((p)->id) >>  8) & 0xFF) == '#') \
                   && (((((p)->id) >> 16) & 0xFF) >= 'A') \
                   && (((((p)->id) >> 16) & 0xFF) <= 'Z') \
                   && (((((p)->id) >> 24) & 0xFF) >= 'A') \
                   && (((((p)->id) >> 24) & 0xFF) <= 'Z') \
                   )

#define isblockMDF3(p) ( ((p) != NULL) \
                   /* block in MDF32: WORD id + WORD length */  \
                   && ((p)->length >= sizeof(WORD)*2)     \
                   && (((((p)->id) >>  0) & 0xFF) >= 'A') \
                   && (((((p)->id) >>  0) & 0xFF) <= 'Z') \
                   && (((((p)->id) >>  8) & 0xFF) >= 'A') \
                   && (((((p)->id) >>  8) & 0xFF) <= 'Z') \
                   )


/*--------------------------------------------------------------------------*/
/*                                                                          */
/* General information                                                      */
/*                                                                          */
/*--------------------------------------------------------------------------*/
/*

   Structure of file:
   ------------------

   The file consists of various blocks.
   It starts with the file identification (IDBLOCK) which always is 64 Bytes long.
   The IDBLOCK must be followed by the file header (HDBLOCK) on Byte position 64.
   Apart from these two blocks, there is no regulation for ordering of blocks.

   Example:

   +-----------------+
   | IDBLOCK         | file identification
   |-----------------|
   | HDBLOCK         | file header
   |-----------------|
   | [FHBLOCK]       | file history block (optional)
   |-----------------|
   | [CHBLOCK]       | channel hierarchy block (optional)
   |-----------------|
   | [ATBLOCK]       | attachment, e.g. reference to external file (optional)
   |-----------------|
   | [EVBLOCK]       | event, e.g. trigger (optional)
   |-----------------|
   | [MDBLOCK]       | meta data, e.g. comment (optional)
   |-----------------|
   | DGBLOCK 1       | data group (1..k)
   | | [MDBLOCK]     |   meta data (optional)
   | | CGBLOCK 11    |   channel group (1..j)
   | | | [MDBLOCK]   |       meta data (optional)
   | | | CNBLOCK 111 |     channel (1..i)
   | | | | TXBLOCK   |       channel name
   | | | | [SIBLOCK] |       source information for channel (optional)
   | | | | [MDBLOCK] |       meta data (optional)
   | | | | [CCBLOCK] |       conversion (optional)
   | | | | [CABLOCK] |       array dependency (optional)
   | | | ...         |
   | | ...           |
   | | DTBLOCK       |   data block with values
   | ...             |
   |-----------------|



   Block structure:
   ----------------

   All blocks except of the IDBLOCK consist of three parts:

   1. They start with a block header, i.e. a description which consists of the following elements
       - id:         the identification of the block (4 upper case ASCII letters, e.g. "##DG")
       - length:     the total number of Bytes of the complete block (all three parts)
       - link_count: the number of elements in the link list (i.e. the number of generic links following the block header)
   2. The base block description is followed by a list of links to other blocks. The list has the length defined in link_count.
      It can be empty (link_count = 0).
   3. The link list is followed by the general block data, i.e. all data elements that are not links.
      The block data can be empty.

   +-----------------+
   | BLOCK_HEADER    |  Block header (not to confuse with header block HDBLOCK)
   |   - id          |
   |   - length      |  sizeof(BLOCK_HEADER) = 24 (fix)
   |   - link_count  |
   |-----------------|
   | [BLOCK_LINKS]   |  Link list (link_count elements, can be empty)
   |   - link[0]     |
   |   - ...         |  sizeof(BLOCK_LINKS) = link_count * sizeof(LINK64)
   |   - link[N]     |
   |-----------------|
   | [BLOCK_DATA]    |  All other data members of the block that are not links.
   |   - member A    |
   |   - member B    |  sizeof(BLOCK_DATA) = length - sizeof(BLOCK_HEADER) - sizeof(BLOCK_LINKS)
   |   - ...         |                     = length - 24 - link_count * sizeof(LINK64)
   +-----------------+

   Since the link list of a certain block can be extended in future MDF versions, each block is described by two structures:
   a) BLOCK_LINKS: the specific link list structure  (structure name:  xxBLOCK_LINKS)
   b) BLOCK_DATA:  the general block data structure  (structure name:  xxBLOCK_DATA)

   As a general rule, the total length of a block in a MDF file must always be equal to the sum of the size of three parts:
   length = sizeof(BLOCK_HEADER) + sizeof(BLOCK_LINKS) + sizeof(BLOCK_DATA)
   The length of BLOCK_DATA must match the size of the respective structures / description (for variable length blocks)
   defined for the MDF version specified in IDBLOCK.

   Please note that a there can be multiple references to the same block (e.g. cn_cn_next and ca_element[i] can refer to the same CNBLOCK).
   In such a case, the referred block must not be duplicated.


   Structure of data block:
   ------------------------

   The BLOCK_DATA part of a data block (DTBLOCK) consists of records.
   A record is a data set for a channel group, i.e. a snapshot of the values of each channel in the channel group
   at a certain time stamp.

   The data block can contain several record types. To distinguish the record types,
   each record starts with a record identification (1, 2, 4 or 8 Bytes) of the associated
   channel group. If the data block can contain only one record type (sorted MDF file, each
   data group only contains one channel group), the record identification can be omitted.

   The ordering of the record types within a data block can be arbitrary, but it is expected
   that for a certain record type time progresses in succeeding records.

   The bit position of a channel value within the record is fixed and will be given by
   parameters cn_byte_offset and cn_bit_offset in channel block.
   All bit positions do not consider the record identification.

   +----+------+-----+-----+-----+-----+
   | id | Ci1  | Ci2 | Ci3 | ... | Cij |
   +----+------+-----+-----+-----+-----+
    ...

   Cij - Channel value. If a time channel exists, it must be first one, i.e. Ci1
   id  - Channel group identification (record identification)
   i   - Channel group numbering
   j   - Channel numbering

   Example:
   Data group with 3 channel groups, group i1 contains 2 channels, i2 contains 3 channels and i3 contains 4 channels

   +----+------+-----+
   | i1 | C11  | C12 |
   +----+------+-----+
   +----+------+-----+-----+-----+
   | i3 | C31  | C32 | C33 | C34 |
   +----+------+-----+-----+-----+
   +----+------+-----+-----+
   | i2 | C21  | C22 | C23 |
   +----+------+-----+-----+
   +----+------+-----+
   | i1 | C11  | C12 |
   +----+------+-----+
   ...
   ...

   Instead of one single DTBLOCK, there can be a sequence of DTBLOCKs. In this case, dg_data points to a data list block (DLBLOCK)
   which defines the sequence of DTBLOCKs. The record addressing then works as if the BLOCK_DATA parts of the individual DTBLOCKs were
   concatenated.

   Signal values with variable length
   ----------------------------------
   The signal value stored in a record must have a fixed size (cn_bit_count). In order to allow signal values with
   a variable length, a signal data block (SDBLOCK) can be used (cn_type == MDF4_CN_TYPE_VLSD)

   If the cn_data link of a CNBLOCK is defined and points to a SDBLOCK,
   then the value encoded in the record consists of an integer value representing the sd_offset.

   sd_offset is counted from the start of BLOCK_DATA of the SDBLOCK.
   At the sd_offset, a 4Byte unsigned Integer value (Intel Byte Order) represents the sd_length, i.e. the number of Bytes of the
   variable length signal data (VLSD) value. The actual value then is stored directly after the sd_length value, i.e. the following
   sd_length Bytes contain the value. The interpretation of the Byte stream depends on cn_data_type:
   MDF4_CN_VAL_STRING_XX   => interpret as string (with specified encoding)
   MDF4_CN_VAL_MIME_SAMPLE => depending on MIME type given in cn_unit
   MDF4_CN_VAL_MIME_STREAM => depending on MIME type given in cn_unit
   MDF4_CN_VAL_BYTE_ARRAY  => simple Byte array

   Instead of one single SDBLOCK, there can be a sequence of SDBLOCKs. In this case, cn_data points to a data list block (DLBLOCK)
   which defines the sequence of SDBLOCKs. The data addressing then works as if the BLOCK_DATA parts of the individual SDBLOCKs were
   concatenated.


   Signal values in an external stream
   -----------------------------------
   If (cn_type == MDF4_CN_TYPE_STREAM_SYNC), then the cn_data link of the CNBLOCK must point to an ATBLOCK.
   The data referenced by the ATBLOCK (external file or embedded data) contains a stream with the signal values.
   The (physical) values in the record for the CNBLOCK are used to synchronize the stream with the other values.
   Usually the sync values are time stamps, but they can also be distance/angle values or an index.

   Currently, CANape supports this only for AVI streams (at_tx_mimetype should be "video/avi").
   The record value then represents the frame time in the AVI stream (in seconds, starting from 0).
*/

/*--------------------------------------------------------------------------*/
/*
**                 Definition of data structures
**
**        IDBLOCK  -  Identification block     (IDentification)
**        HDBLOCK  -  Header block             (HeaDer)
**        MDBLOCK  -  Meta data block (XML)    (Meta Data)
**        TXBLOCK  -  Text block               (TeXt)
**        FHBLOCK  -  File history block       (File History)
**        CHBLOCK  -  Hierarchy block          (Channel Hierarchy)
**        ATBLOCK  -  Attachment block         (ATachment)
**        EVBLOCK  -  Event block              (EVent)
**        DGBLOCK  -  Data group block         (Data Group)
**        CGBLOCK  -  Channel group block      (Channel Group)
**        SIBLOCK  -  Source information block (Source Information)
**        CNBLOCK  -  Channel block            (ChaNnel)
**        CCBLOCK  -  Conversion block         (Channel Conversion)
**        CABLOCK  -  Array block              (Channel Array)
**        DTBLOCK  -  Data block               (DaTa)
**        SRBLOCK  -  Sample reduction block   (Sample Reduction)
**        RDBLOCK  -  Reduction data block     (Reduction Data)
**        SDBLOCK  -  Signal data block        (Signal Data)
**        DLBLOCK  -  Data list block          (Data List)
**        DZBLOCK  -  Zipped data block        (Data Zipped)
**        HLBLOCK  -  Data list header block   (Header of List)
**        LDBLOCK  -  List data block          (List Data)
**        DVBLOCK  -  Data value block         (Data Value)
**        DIBLOCK  -  Data invalidation block  (Data Invalidation)
**        RVBLOCK  -  Reduction data value block (Reduction Value)
**        RIBLOCK  -  Reduction data invalidation block (Reduction Invalidation)
*/
/*--------------------------------------------------------------------------*/

/* ######################################################################## */

/* General defines which apply to more than one block */

#define MDF4_MIN_TO_NS(_t_)  ((__int64)(_t_) * 60 * 1000000000)
#define MDF4_MIN_TO_HRS(_t_) ((_t_)/60)
#define MDF4_HRS_TO_MIN(_t_) ((_t_)*60)

#define MDF4_SYNC_NONE             0   /* no synchronization                                */
#define MDF4_SYNC_TIME             1   /* synchronized with time stamp in seconds           */
#define MDF4_SYNC_ANGLE            2   /* synchronized with angle value in radians          */
#define MDF4_SYNC_DISTANCE         3   /* synchronized with distance value in meters        */
#define MDF4_SYNC_INDEX            4   /* synchronized with zero-based record index value   */

#define MDF4_TIME_FLAG_LOCAL_TIME    (1<<0)   /* start time stamp in ns is not UTC time, but local time */
#define MDF4_TIME_FLAG_OFFSETS_VALID (1<<1)   /* time zone and DST offsets are valid                    */


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* IDBLOCK64                                                                */
/* File Identification Header                                               */
/* Identification of file                                                   */

#define MDF4_ID_LENGTH         64             /* length IDBLOCK64 (fix)            */

#define MDF4_ID_FILE           8              /* length file format identification */
#define MDF4_ID_VERS           8              /* length version identification     */
#define MDF4_ID_PROG           8              /* length program identification     */

#define MDF4_ID_FILE_STRING    "MDF         " /* file format identification        */
#define MDF4_ID_VERS_STRING    "4.20    "     /* version identification            */
#define MDF4_ID_VERS_NO        420            /* format version number             */
#define MDF4_ID_PROG_STRING    "........"     /* program identification            */

// old version defines to create old file versions
#define MDF4_ID_VERS_STRING_400   "4.00    "
#define MDF4_ID_VERS_NO_400       400

#define MDF4_ID_VERS_STRING_410   "4.10    "
#define MDF4_ID_VERS_NO_410       410

#define MDF4_ID_VERS_STRING_411   "4.11    "
#define MDF4_ID_VERS_NO_411       411

#define MDF4_ID_VERS_STRING_420   "4.20    "
#define MDF4_ID_VERS_NO_420       420

#define MDF4_ID_UNFINALIZED                            "UnFinMF " /* file format identification for unfinalized MDF */

#define MDF4_ID_UNFIN_FLAG_INVAL_CYCLE_COUNT_CG        (1<<0)  /* Cycle count of CG/CA block is not valid */
#define MDF4_ID_UNFIN_FLAG_INVAL_CYCLE_COUNT_SR        (1<<1)  /* Cycle count of SR block is not valid */
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_DT           (1<<2)  /* Length of last DT block of chained list is not valid (extend until next block or EOF) */
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_RD           (1<<3)  /* Length of last RD block of chained list is not valid (extend until next block or EOF) */
#define MDF4_ID_UNFIN_FLAG_INVAL_LEN_LAST_DL           (1<<4)  /* Length of last DL block in chained list is not valid, may contain NIL links */
#define MDF4_ID_UNFIN_FLAG_INVAL_VLSD_CG_SD_LEN        (1<<5)  /* Member cg_sdblock_length of VLSD CG block is not valid (usually 0) */
#define MDF4_ID_UNFIN_FLAG_INVAL_UNSORTED_VLSD_OFFSET  (1<<6)  /* VLSD offset in fixed-length record for unsorted DG is invalid (usually 0) */

// custom unfinalized flags for Vector
#define MDF_ID_UNFIN_FLAG_CUSTOM_INVERSE_CHAIN_DL        (1<<0) // inverse chain of DL blocks for faster writing of MF4 (CANape, MDF4 Lib)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_CANAPE     (1<<1) // temp files in CANape (DGx)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_MDF4LIB    (1<<2) // temp files in MDF4Lib (link as file name)
#define MDF_ID_UNFIN_FLAG_CUSTOM_TEMP_FILE_DG_MDF4LIB_EX (1<<3) // temp files in MDF4Lib with only temp files
#define MDF_ID_UNFIN_FLAG_CUSTOM_RING_BUFFER             (1<<4) // MDF ring buffer used in CANape, DT blocks must be ordered according to time stamp of first record

//   For compatibility with earlier MDF versions, all strings in the IDBLOCK must be given as
//   single Byte character (SBC) strings in plain ASCII (ISO-8859-1) encoding.

typedef struct idblock64 {                    /* Identification block */
                                              /* -------------------- */

  BYTE        id_file[MDF4_ID_FILE];   /* 8*/ /* file identification     (always MDF4_ID_FILE,  no \0 termination, always SBC string) */
  BYTE        id_vers[MDF4_ID_VERS];   /* 8*/ /* format identification   (MDF4_ID_VERS_STRING,  no \0 termination, always SBC string) */
  BYTE        id_prog[MDF4_ID_PROG];   /* 8*/ /* program identification  (e.g. "CANape", etc,   no \0 termination, always SBC string) */
  BYTE        id_reserved1[4];         /* 4*/ /* reserved, must be 0 */
  WORD        id_ver;                  /* 2*/ /* default version number  (MDF4_ID_VERS_NO) */
  BYTE        id_reserved2[30];        /*30*/ /* reserved, must be filled with 0 */
  WORD        id_unfin_flags;          /* 2*/ /* bit combination of MDF4_ID_UNFIN_FLAG_xxx flags indicating required steps for finalization, must be 0 for finalized MDF */
  WORD        id_custom_unfin_flags;   /* 2*/ /* bit combination of custom flags indicating required steps for finalization, must be 0 for finalized MDF */
} IDBLOCK64;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* HDBLOCK                                                                  */
/* Header block                                                             */
/* General information about the complete file                              */

#define MDF4_HD_TIME_SRC_PC         0       /* local PC reference time (Default) */
#define MDF4_HD_TIME_SRC_EXTERNAL   10      /* external time source */
#define MDF4_HD_TIME_SRC_ABS_SYNC   16      /* external absolute synchronized time */

#define MDF4_HD_ID              GENERATE_ID('H', 'D')   /* Identification */
#define MDF4_HD_MIN_LENGTH      (sizeof(BLOCK_HEADER) + sizeof(HDBLOCK_LINKS) + sizeof(HDBLOCK_DATA))
#define MDF4_HD_MIN_LINK_COUNT  (sizeof(HDBLOCK_LINKS)/sizeof(LINK64))

#define MDF4_HD_FLAG_ANGLE_VALID        (1<<0)   /* start angle is valid    */
#define MDF4_HD_FLAG_DISTANCE_VALID     (1<<1)   /* start distance is valid */

typedef struct hdblock_links
{
  LINK64       hd_dg_first;          /* pointer to (first) data group block        (DGBLOCK) */
  LINK64       hd_fh_first;          /* pointer to (first) file history block      (FHBLOCK) */
  LINK64       hd_ch_tree;           /* pointer to (first) channel hierarchy block (CHBLOCK) (can be NIL) */
  LINK64       hd_at_first;          /* pointer to (first) attachment              (ATBLOCK) (can be NIL) */
  LINK64       hd_ev_first;          /* pointer to (first) event block             (EVBLOCK) (can be NIL) */
  LINK64       hd_md_comment;        /* pointer to measurement file comment        (MDBLOCK or TXBLOCK) (can be NIL) */
                                     /* XML schema for contents of MDBLOCK see hd_comment.xsd */
} HDBLOCK_LINKS;

typedef struct hdblock_data
{
  QWORD        hd_start_time_ns;     /*8*/  /* start of measurement in ns elapsed since 00:00 Jan 1, 1970 UTC time or local time
                                              (if MDF4_TIME_FLAG_LOCAL_TIME flag set) */
  SWORD        hd_tz_offset_min;     /*2*/  /* time zone offset in minutes (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) */
                                            /* i.e. local_time_ns = hd_start_time_ns + MDF4_MIN_TO_NS(hd_tz_offset_min) */
  SWORD        hd_dst_offset_min;    /*2*/  /* DST offset for local time in minutes used at start time (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) */
                                            /* i.e. local_DST_time_ns = hd_start_time_ns + MDF4_MIN_TO_NS(hd_tz_offset_min) + MDF4_MIN_TO_NS(hd_dst_offset_min) */
  BYTE         hd_time_flags;        /*1*/  /* time flags are bit combination of [MDF4_TIME_FLAG_xxx] */
  BYTE         hd_time_class;        /*1*/  /* time quality class [MDF4_HD_TIME_SRC_xxx] */
  BYTE         hd_flags;             /*1*/  /* flags are bit combination of [MDF4_HD_FLAG_xxx] */
  BYTE         hd_reserved;          /*1*/  /* reserved */
  REAL         hd_start_angle_rad;   /*8*/  /* start angle value in rad at start of measurement (for angle synchronization) */
  REAL         hd_start_distance_m;  /*8*/  /* start distance value in meters at start of measurement (for distance synchronization) */
} HDBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* MDBLOCK                                                                  */
/* Meta Data block                                                          */
/* Meta data is given as UTF-8 encoded XML string, see separate description */
/* For simplicity and faster processing, a link to a MDBLOCK can be replaced
   with a link to a TXBLOCK with the contents of <TX> tag if no other
   information is required. */

#define MDF4_MD_ID          GENERATE_ID('M', 'D')   /* Identification */
#define MDF4_MD_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_MD_LINK_COUNT  (0)

typedef struct mdblock_data
{
  /*
               UTF-8 encoded string, string must be zero terminated,
               line break (in CDATA) with \r\n (CR + LF)
  BYTE         md_data[BLOCK_HEADER.length - sizeof(BLOCK_HEADER)];
  */
    char s[1];
} MDBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* TXBLOCK                                                                  */
/* Text block                                                               */
/* text is given as UTF-8 encoded string                                    */

#define MDF4_TX_ID          GENERATE_ID('T', 'X')   /* Identification */
#define MDF4_TX_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_TX_LINK_COUNT  (0)

typedef struct txblock_data
{
  /*
                 UTF-8 encoded string, string must be zero terminated,
                 line break with \r\n (CR + LF)
  BYTE           tx_data[BLOCK_HEADER.length - sizeof(BLOCK_HEADER)];
  */
    char s[1];
} TXBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* FHBLOCK                                                                  */
/* File history block                                                       */
/* Log entries for change history of the MDF file                           */
/* First block gives detailed information about the generating application  */

#define MDF4_FH_ID              GENERATE_ID('F', 'H')   /* Identification */
#define MDF4_FH_MIN_LENGTH      (sizeof(BLOCK_HEADER) + sizeof(FHBLOCK_LINKS) + sizeof(FHBLOCK_DATA))
#define MDF4_FH_MIN_LINK_COUNT  (sizeof(FHBLOCK_LINKS)/sizeof(LINK64))

typedef struct fhblock_links
{
  LINK64       fh_fh_next;                  /* pointer to (next) file history block (FHBLOCK) (can be NIL) */
  LINK64       fh_md_comment;               /* pointer to comment and further info about this log entry (MDBLOCK) */
                                            /* XML schema for contents of MDBLOCK see fh_comment.xsd */
} FHBLOCK_LINKS;

typedef struct fhblock_data
{
  QWORD        fh_time_ns;           /*8*/  /* Time stamp at which the file has been changed/created
                                               in ns elapsed since 00:00 Jan 1, 1970 UTC time or local time
                                              (if MDF4_TIME_FLAG_LOCAL_TIME flag set) */
  SWORD        fh_tz_offset_min;     /*2*/  /* time zone offset in minutes (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) */
                                            /* i.e. local_time_ns = fh_change_time_ns + MDF4_MIN_TO_NS(fh_tz_offset_min) */
  SWORD        fh_dst_offset_min;    /*2*/  /* DST offset for local time in minutes used at start time (only valid if MDF4_TIME_FLAG_OFFSETS_VALID is set) */
                                            /* i.e. local_DST_time_ns = fh_change_time_ns + MDF4_MIN_TO_NS(fh_tz_offset_min) + MDF4_MIN_TO_NS(fh_dst_offset_min) */
  BYTE         fh_time_flags;        /*1*/  /* time flags are bit combination of [MDF4_TIME_FLAG_xxx] */
  BYTE         fh_reserved[3];       /*3*/  /* reserved */

} FHBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* CHBLOCK                                                                  */
/* Channel Hierarchy block                                                  */
/* The channel hierarchy blocks forms a first-child/next-sibling tree       */
/* structure of hierarchy levels with information about the grouping of     */
/* channels/signals (similar to ASAP2 groups)                               */

#define MDF4_CH_ID              GENERATE_ID('C', 'H')   /* Identification */
#define MDF4_CH_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 5*8)
#define MDF4_CH_MIN_LINK_COUNT  ((sizeof(HDBLOCK_LINKS)-sizeof(CHANNEL64))/sizeof(LINK64))

#define MDF4_CH_TYPE_GROUP             0      /* all elements and childs of this hierarchy level form a logical group
                                                 (see ASAP2 V1.6 keyword GROUP)
                                                 All descendant CHBLOCK childs must be of one of the following types:
                                                 MDF4_CH_TYPE_GROUP/MDF4_CH_TYPE_FUNCTION/MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST
                                              */
#define MDF4_CH_TYPE_FUNCTION          1      /* all descendant childs of this hierarchy level form a functional group
                                                 (see ASAP2 V1.6 keyword FUNCTION)
                                                 elements (channel references) are not allowed
                                                 All descendant CHBLOCK childs must be of one of the following types:
                                                 MDF4_CH_TYPE_FUNCTION/MDF4_CH_TYPE_MEAS_INPUT/MDF4_CH_TYPE_MEAS_OUTPUT/
                                                 MDF4_CH_TYPE_MEAS_LOCAL/MDF4_CH_TYPE_CAL_DEF/MDF4_CH_TYPE_CAL_REF.
                                                 Except of MDF4_CH_TYPE_FUNCTION, all other types must only occur once within the child list.
                                              */
#define MDF4_CH_TYPE_STRUCTURE         2      /* all elements and childs of this hierarchy level form a (logical) structure,
                                                 e.g. for C/C++ keyword struct.
                                                 Note: not to be used parallel to a physical structure (cn_composition references
                                                 linked list of CNBLOCKs with members of structure).
                                                 The logical structure should only be used where a physical structure definition is not applicable,
                                                 i.e. if elements of structure cannot be stored in same record (because measured with different rates).
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */
#define MDF4_CH_TYPE_MAP_LIST          3      /* all elements of this hierarchy level form a map list (see ASAP2 V1.6 keyword MAP_LIST):
                                                 the first element represents the z axis (must be a curve / CABLOCK of type MDF4_CA_ARRAY_TYPE_AXIS)
                                                 all other elements represent the maps (CABLOCK of type MDF4_CA_ARRAY_TYPE_LOOKUP)
                                                 that form the map list "cuboid"
                                                 Descendant CHBLOCK childs are not allowed, i.e. ch_ch_first_child must be NIL.
                                              */

#define MDF4_CH_TYPE_MEAS_INPUT        4      /* all elements and childs of this hierarchy level are input variable of this function
                                                 (see ASAP2 V1.6 keyword IN_MEASUREMENT for FUNCTION)
                                                 referenced channels must be measurement object (MDF4_CN_FLAG_CALIBRATION bit not set)
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */
#define MDF4_CH_TYPE_MEAS_OUTPUT       5      /* all elements and childs of this hierarchy level are output variables of this function
                                                 (see ASAP2 V1.6 keyword OUT_MEASUREMENT for FUNCTION)
                                                 referenced channel must be measurement object (MDF4_CN_FLAG_CALIBRATION bit not set)
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */
#define MDF4_CH_TYPE_MEAS_LOCAL        6      /* all elements and childs of this hierarchy level are local variables of this function
                                                 (see ASAP2 V1.6 keyword LOC_MEASUREMENT for FUNCTION)
                                                 referenced channel must be measurement object (MDF4_CN_FLAG_CALIBRATION bit not set)
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */
#define MDF4_CH_TYPE_CAL_DEF           7      /* all elements and childs of this hierarchy level are calibration objects defined in this function
                                                 (see ASAP2 V1.6 keyword DEF_CHARACTERISTIC for FUNCTION)
                                                 referenced channel must be calibration object (MDF4_CN_FLAG_CALIBRATION bit set)
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */
#define MDF4_CH_TYPE_CAL_REF           8      /* all elements and childs of this hierarchy level are calibration objects referenced in this function
                                                 (see ASAP2 V1.6 keyword REF_CHARACTERISTIC for FUNCTION)
                                                 referenced channel must be calibration object (MDF4_CN_FLAG_CALIBRATION bit set)
                                                 All descendant CHBLOCK childs must be of of one of the following types:
                                                 MDF4_CH_TYPE_STRUCTURE/MDF4_CH_TYPE_MAP_LIST.
                                              */

typedef struct chblock_links
{
  LINK64       ch_ch_next;                  /* pointer to next sibling channel hierarchy block (CHBLOCK) (can be NIL) */
  LINK64       ch_ch_first;                 /* pointer to first child  channel hierarchy block (CHBLOCK) (can be NIL) */
  LINK64       ch_tx_name;                  /* pointer to TXBLOCK with name of the hierarchy level       (can be NIL) */
  LINK64       ch_md_comment;               /* pointer to MD/TXBLOCK with comment and other info of the hierarchy level (can be NIL) */
                                            /* XML schema for contents of MDBLOCK see ch_comment.xsd */
  CHANNEL64    ch_element[1];               /* variable length list with pointers to LINK triples (CHANNEL64 structure).
                                               The list has ch_element_count elements, but can also be empty.
                                               Each LINK triple references a channel associated to this hierarchy level.
                                               Note that a channel can occur in more than one hierarchy level.
                                            */
} CHBLOCK_LINKS;

typedef struct chblock_data
{
  DWORD        ch_element_count;     /*4*/  /* number of elements (channel references) contained in this hierarchy level */
  BYTE         ch_type;              /*1*/  /* type of the hierarchy level [MDF4_CH_TYPE_xxx] */
  BYTE         ch_reserved[3];       /*3*/  /* reserved */
} CHBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* ATBLOCK                                                                  */
/* Attachment block                                                         */
/* Either a reference to an external file or embedded binary data           */

#define MDF4_AT_ID              GENERATE_ID('A', 'T')  /* Identification */
#define MDF4_AT_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 9*8)
#define MDF4_AT_MIN_LINK_COUNT  (sizeof(ATBLOCK_LINKS)/sizeof(LINK64))

#define MDF4_AT_FLAG_EMBEDDED     (1<<0)   /* block contains the plain data of the attached file (at_tx_filename contains original file name)
                                              otherwise, the attachment data is contained in an external file (at_tx_filename contains file path and name)
                                              for external file, MD5 check sum should be used to ensure that file was not changed
                                           */
#define MDF4_AT_FLAG_COMPRESSED   (1<<1)   /* embedded data is compressed with gzip (equal to MIME gzip compression flag). Only valid if MDF4_AT_FLAG_EMBEDDED is set!
                                              MD5 check sum can be used to ensure that decompression returns same result.
                                           */
#define MDF4_AT_FLAG_MD5_VALID    (1<<2)   /* MD5 check sum in at_md5_checksum is valid. */


typedef struct atblock_links
{
  LINK64        at_at_next;              /* pointer to next attachment block (ATBLOCK), can be NIL */
  LINK64        at_tx_filename;          /* pointer to a TXBLOCK with file name, can be NIL if embedded data */
  LINK64        at_tx_mimetype;          /* pointer to a TXBLOCK with content-type text of MIME type, can be NIL if unknown */
  LINK64        at_md_comment;           /* pointer to a TXBLOCK/MDBLOCK with additional comments, can be NIL */
                                         /* XML schema for contents of MDBLOCK see at_comment.xsd */
} ATBLOCK_LINKS;

typedef struct atblock_data
{
  WORD          at_flags;             /* 2*/ /* flags are bit combination of [MDF4_AT_FLAG_xxx] */
  WORD          at_creator_index;     /* 2*/ /* zero-based index of FHBLOCK in list starting at hd_fh_first
                                                with reference to application that created/modified the attachment */
  BYTE          at_reserved[4];       /* 4*/ /* reserved */
  BYTE          at_md5_checksum[16];  /*16*/ /* MD5 check sum over external file or uncompressed embedded data */
  QWORD         at_original_size;     /* 8*/ /* original data size in Bytes (external file or uncompressed embedded data) */
  QWORD         at_embedded_size;     /* 8*/ /* size of embedded data in Bytes (zero for external file) */
  /*
  plain data:
  BYTE     at_embedded_data[at_embedded_size];
  */

} ATBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* EVBLOCK                                                                  */
/* Event block                                                              */
/* Information about a single event that occurred during measurement        */
/* Events are stored as a linked list.                                      */
/* Each event optionally can have a companion stop event                    */

#define MDF4_EV_ID              GENERATE_ID('E', 'V')   /* Identification */
#define MDF4_EV_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 9*8)
#define MDF4_EV_MIN_LINK_COUNT  (sizeof(EVBLOCK_LINKS) / sizeof(LINK64)-1)

#define MDF4_EV_TYPE_RECORDING          0   /* recording event               (only range)     */
#define MDF4_EV_TYPE_REC_INTERRUPT      1   /* recording interrupt           (point or range) */
#define MDF4_EV_TYPE_ACQ_INTERRUPT      2   /* acquisition interrupt         (point or range) */
#define MDF4_EV_TYPE_TRIGGER_REC_START  3   /* recording start trigger event (only point)     */
#define MDF4_EV_TYPE_TRIGGER_REC_STOP   4   /* recording stop trigger event  (only point)     */
#define MDF4_EV_TYPE_TRIGGER            5   /* trigger event                 (only point)     */
#define MDF4_EV_TYPE_MARKER             6   /* marker event                  (point or range) */

#define MDF4_EV_SYNC_TIME             MDF4_SYNC_TIME       /* calculated synchronization value represents time stamp in seconds           */
#define MDF4_EV_SYNC_ANGLE            MDF4_SYNC_ANGLE      /* calculated synchronization value represents angle value in radians          */
#define MDF4_EV_SYNC_DISTANCE         MDF4_SYNC_DISTANCE   /* calculated synchronization value represents distance value in meters        */
#define MDF4_EV_SYNC_INDEX            MDF4_SYNC_INDEX      /* calculated synchronization value represents zero-based record index value   */

#define MDF4_EV_RANGE_NONE            0     /* point          */
#define MDF4_EV_RANGE_BEGIN           1     /* begin of range */
#define MDF4_EV_RANGE_END             2     /* end of range   */

#define MDF4_EV_CAUSE_OTHER           0     /* other cause or unknown                    */
#define MDF4_EV_CAUSE_ERROR           1     /* event caused by error                     */
#define MDF4_EV_CAUSE_TOOL            2     /* event caused by tool / internal condition */
#define MDF4_EV_CAUSE_SCRIPT          3     /* event caused by scripting command         */
#define MDF4_EV_CAUSE_USER            4     /* event caused by user interaction          */

#define MDF4_EV_FLAG_POST_PROCESSING  (1<<0)   /* event has been generated by some post processing step */
#define MDF4_EV_FLAG_GROUP_NAME       (1<<1)   /* event has a group name (ev_tx_group_name) (since MDF 4.2) */

#define ev_ev_parent ev_parent /* until MDF 4.1 */

typedef struct evblock_links               /* Event block links */
{
  LINK64       ev_ev_next;                 /* pointer to next event block (EVBLOCK),  can be NIL */
  LINK64       ev_parent;                  /* pointer to parent event (EVBLOCK) or event signal group (CGBLOCK) with parent event signal, can be NIL */
  LINK64       ev_ev_range;                /* pointer to range begin event (EVBLOCK), can be NIL, must be NIL for ev_range_type != 2 */
  LINK64       ev_tx_name;                 /* pointer to TXBLOCK with event name,     can be NIL */
  LINK64       ev_md_comment;              /* pointer to TX/MDBLOCK with comment and additional information, can be NIL */
                                           /* XML schema for contents of MDBLOCK see ev_comment.xsd */
  LINK64       ev_scope[1];                /* list with references to CNBLOCKs or CGBLOCKs, size specified in ev_scope_count */
  //LINK64       ev_at_reference[1];       /* list with references to ATBLOCKs, size specified in ev_attachment_count */
  //LINK64       ev_tx_group_name;         /* pointer to TXBLOCK with event group name (since MDF 4.2), can be NIL, must be NIL for template EVBLOCK  */
                                           /* Attention: does only exist if MDF4_EV_FLAG_GROUP_NAME (bit 1) is set in ev_flags */
} EVBLOCK_LINKS;

typedef struct evblock_data                /* Event block data */
{
  BYTE         ev_type;              /*1*/ /* event type [MDF4_EV_TYPE_xxx] */
  BYTE         ev_sync_type;         /*1*/ /* sync type  [MDF4_EV_SYNC_xxx] */
  BYTE         ev_range_type;        /*1*/ /* range type [MDF4_EV_RANGE_xxx] */
  BYTE         ev_cause;             /*1*/ /* cause of event [MDF4_EV_CAUSE_xxx] */
  BYTE         ev_flags;             /*1*/ /* flags are bit combination of [MDF4_EV_FLAG_xxx] */
  BYTE         ev_reserved[3];       /*3*/ /* reserved */
  DWORD        ev_scope_count;       /*4*/ /* size of ev_scope list. Can be 0 */
  WORD         ev_attachment_count;  /*2*/ /* number of attachments related to this event, i.e. size of ev_at_references. Can be 0. */
  WORD         ev_creator_index;     /*2*/ /* zero-based index of FHBLOCK in list starting at hd_fh_first
                                              with reference to application that created/modified the event */
  INT64        ev_sync_base_value;   /*8*/ /* base value for calculation of synchronization value for event */
                                           /* unit of synchronization value depends on ev_sync_type */
  REAL         ev_sync_factor;       /*8*/ /* factor for calculation of synchronization value for event */

} EVBLOCK_DATA;


 /* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DGBLOCK                                                                  */
/* Data group block                                                         */
/* Information about a data group                                           */

#define MDF4_DG_ID              GENERATE_ID('D', 'G')   /* Identification */
#define MDF4_DG_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 5*8)
#define MDF4_DG_MIN_LINK_COUNT  (sizeof(DGBLOCK_LINKS) / sizeof(LINK64))

typedef struct dgblock_links
{
  LINK64       dg_dg_next;                 /* pointer to (next)  data group block    (DGBLOCK) (can be NIL) */
  LINK64       dg_cg_first;                /* pointer to (first) channel group block (CGBLOCK)              */
  LINK64       dg_data;                    /* pointer to         data list block (DLBLOCK/HZBLOCK/LDBLOCK) or a data block (DTBLOCK, DVBLOCK or respective DZBLOCK) (can be NIL) */
  LINK64       dg_md_comment;              /* pointer to         TXBLOCK/MDBLOCK with additional comments (can be NIL) */
                                           /* XML schema for contents of MDBLOCK see dg_comment.xsd */
} DGBLOCK_LINKS;

typedef struct dgblock_data
{
  BYTE         dg_rec_id_size;     /*1*/   /* Number of Bytes used for record ID [0,1,2,4,8] (at start of record) */
  BYTE         dg_reserved[7];     /*7*/   /* reserved */
} DGBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* CGBLOCK                                                                  */
/* channel group block                                                      */
/* Information about a group of channels with equal time raster             */

#define MDF4_CG_ID              GENERATE_ID('C', 'G')   /* Identification */
#define MDF4_CG_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 10*8)
#define MDF4_CG_MIN_LINK_COUNT  (sizeof(CGBLOCK_LINKS) / sizeof(LINK64))

#define MDF4_CG_FLAG_VLSD            (1<<0) /* channel group describes a channel with variable length signal data (VLSD) */
#define MDF4_CG_FLAG_BUS_EVENT       (1<<1) /* channel group contains bus event
                                               for bus message logging (since MDF 4.1), see association standard on bus logging */
#define MDF4_CG_FLAG_PLAIN_BUS_EVENT (1<<2) /* channel group contains plain bus event without signal descriptions for payload (only valid if MDF4_CG_FLAG_BUS_EVENT is set)
                                               for bus message logging (since MDF 4.1), see association standard on bus logging */
#define MDF4_CG_FLAG_REMOTE_MASTER   (1<<3) /* cg_cg_master points to another channel group which contains the master channel(s) for this group (since MDF 4.2)*/
#define MDF4_CG_FLAG_EVENT_SIGNAL    (1<<4) /* channel group contains event signal struct, i.e. is for storing events (since MDF 4.2)*/

typedef struct cgblock_links
{
  LINK64       cg_cg_next;                 /* pointer to next channel group block (CGBLOCK)       (can be NIL)  */
  LINK64       cg_cn_first;                /* pointer to first channel block (CNBLOCK)            (            must be NIL if MDF4_CG_FLAG_VLSD flag is set) */
  LINK64       cg_tx_acq_name;             /* pointer to TXBLOCK with acquisition name            (can be NIL, must be NIL if MDF4_CG_FLAG_VLSD flag is set) */
  LINK64       cg_si_acq_source;           /* pointer to SIBLOCK with acquisition source info     (can be NIL, must be NIL if MDF4_CG_FLAG_VLSD flag is set) */
  LINK64       cg_sr_first;                /* pointer to first sample reduction block (SRBLOCK)   (can be NIL, must be NIL if MDF4_CG_FLAG_VLSD flag is set) */
  LINK64       cg_md_comment;              /* pointer to TXBLOCK/MDBLOCK with additional comments (can be NIL, must be NIL if MDF4_CG_FLAG_VLSD flag is set) */
                                           /* XML schema for contents of MDBLOCK see cg_comment.xsd */
  //LINK64       cg_cg_master;             /* pointer to another CGBLOCK containing master channel (since MDF 4.2) */
                                           /* Attention: does only exist if MDF4_CG_FLAG_REMOTE_MASTER (bit 3) is set in cg_flags.*/
} CGBLOCK_LINKS;

typedef struct cgblock_data
{
  QWORD        cg_record_id;       /*8*/   /* Record identification, can be up to 8 Bytes long */
  QWORD        cg_cycle_count;     /*8*/   /* Number of cycles, i.e. number of samples for this channel group */
  WORD         cg_flags;           /*2*/   /* Flags are bit combination of [MDF4_CG_FLAG_xxx] */
  WORD         cg_path_separator;  /*2*/   /* Value of character as UTF16-LE to be used as path separator, 0 if no path separator specified. (since MDF 4.1) */
  BYTE         cg_reserved[4];     /*4*/   /* reserved */
  union  {
    QWORD cg_sdblock_length;       /*8*/   /* Total length in Bytes of variable length signal values for VLSD CGBLOCK. */
    struct  {
      DWORD cg_data_bytes;         /*4*/   /* Length of data range in record in Bytes */
      DWORD cg_inval_bytes;        /*4*/   /* Length of invalidation range in record in Bytes */
    } cg_record_bytes;
  };

} CGBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* SIBLOCK                                                                  */
/* Source information block                                                 */
/* Information about a the source of a channel or an acquisition rate       */

#define MDF4_SI_ID              GENERATE_ID('S', 'I')   /* Identification */
#define MDF4_SI_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 4*8)
#define MDF4_SI_MIN_LINK_COUNT  (sizeof(SIBLOCK_LINKS) / sizeof(LINK64))


#define MDF4_SI_TYPE_OTHER         0       /* source type is unknown */
#define MDF4_SI_TYPE_ECU           1       /* source is a ECU */
#define MDF4_SI_TYPE_BUS           2       /* source is a bus */
#define MDF4_SI_TYPE_IO            3       /* source is a I/O device */
#define MDF4_SI_TYPE_TOOL          4       /* source is a software tool */
#define MDF4_SI_TYPE_USER          5       /* source is a user input/user interaction */

#define MDF4_SI_BUS_NONE           0       /* no bus */
#define MDF4_SI_BUS_OTHER          1       /* bus type unknown or it is none of the enum, bus type name can be given in <bus_type> tag in si_md_source_comment */
#define MDF4_SI_BUS_CAN            2       /* CAN */
#define MDF4_SI_BUS_LIN            3       /* LIN */
#define MDF4_SI_BUS_MOST           4       /* MOST */
#define MDF4_SI_BUS_FLEXRAY        5       /* FlexRay */
#define MDF4_SI_BUS_K_LINE         6       /* K-Line */
#define MDF4_SI_BUS_ETHERNET       7       /* Ethernet */
#define MDF4_SI_BUS_USB            8       /* USB */


#define MDF4_SI_FLAG_SIMULATION   (1<<0)   /* source is only simulated. Must not be set for si_type = 4 */

typedef struct siblock_links
{
  LINK64       si_tx_name;                 /* pointer to TXBLOCK    with source name     (can be NIL) */
  LINK64       si_tx_path;                 /* pointer to TXBLOCK    with source path     (can be NIL) */
  LINK64       si_md_comment;              /* pointer to MD/TXBLOCK with source comment  (can be NIL) */
} SIBLOCK_LINKS;

typedef struct siblock_data
{
  BYTE         si_type;            /*1*/   /* type of source [MDF4_SI_TYPE_xxx] */
  BYTE         si_bus_type;        /*1*/   /* type of data bus [MDF4_SI_BUS_xxx] */
  BYTE         si_flags;           /*1*/   /* flags are bit combination of [MDF4_SI_FLAG_xxx] */
  BYTE         si_reserved[5];     /*5*/   /* reserved */
} SIBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* CNBLOCK                                                                  */
/* channel block                                                            */
/* Information about a channel                                              */

#define MDF4_CN_ID              GENERATE_ID('C', 'N')   /* Identification */
#define MDF4_CN_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 17*8)
#define MDF4_CN_MIN_LINK_COUNT  (sizeof(CNBLOCK_LINKS) / sizeof(LINK64))

#define MDF4_CN_TYPE_VALUE            0       /* fixed length signal data value channel (contained in record)
                                              */
#define MDF4_CN_TYPE_VLSD             1       /* variable length signal data (VLSD) channel
                                                 record contains offset of VLSD values in SDBLOCK (starting with UINT32 length value)
                                                 => cn_data must point to SDBLOCK or DZBLOCK or DLBLOCK/HLBLOCK
                                                 => record value is of type MDF4_CN_VAL_UNSIGN_INTEL with cn_bit_count bits
                                                 => cn_data_type specifies which data is stored in the SDBLOCK data section
                                                 => cn_cc_conversion must be applied to data stored in SDBLOCK
                                              */
#define MDF4_CN_TYPE_MASTER           2       /* master channel for all channels of channel group for respective sync type
                                                 physical value must have the SI unit defined by sync type (with or without CC rule).
                                                 values of this channel listed over the record index must be monotonic increasing.
                                              */
#define MDF4_CN_TYPE_VIRTUAL_MASTER   3       /* like master channel except that record does not contain value (cn_bit_count must be 0).
                                                 instead the physical values are calculated by feeding the zero-based record index to the conversion rule
                                              */

#define MDF4_CN_TYPE_STREAM_SYNC      4       /* record contains values to synchronize with other stream
                                                 => data must point to ATBLOCK in main list of ATBLOCKs
                                                 ATBLOCK must be evaluated how to extract the data
                                                 physical value must have the SI unit defined by sync type (with or without CC rule).
                                              */
#define MDF4_CN_TYPE_MLSD             5       /* maximum length signal data (MLSD) channel
                                                 equal to a fixed length signal data value channel (MDF4_CN_TYPE_VALUE),
                                                 except that the actual number of Bytes in the record is given by a size signal
                                                 => cn_data must point to CNBLOCK of same channel group with size signal
                                                 => phys value of size signal gives the number of Bytes(!) for the MLSD channel
                                                 => phys value of size signal always must be less or equal to (8 * cn_bit_count)
                                                 (since MDF 4.1)
                                              */

#define MDF4_CN_TYPE_VIRTUAL_DATA     6       /* virtual data channel (cn_bit_count = 0, raw value = record index like for MDF4_CN_TYPE_VIRTUAL_MASTER)
                                                 (since MDF 4.1)
                                              */


#define MDF4_CN_SYNC_NONE           MDF4_SYNC_NONE       /* physical values are according to unit / conversion rule  */
#define MDF4_CN_SYNC_TIME           MDF4_SYNC_TIME       /* physical values are time stamps in seconds               */
#define MDF4_CN_SYNC_ANGLE          MDF4_SYNC_ANGLE      /* physical values are angle values in radians              */
#define MDF4_CN_SYNC_DISTANCE       MDF4_SYNC_DISTANCE   /* physical values are distance value in meters             */
#define MDF4_CN_SYNC_INDEX          MDF4_SYNC_INDEX      /* physical values are index values (integer, no dimension) */

#define MDF4_CN_VAL_UNSIGN_INTEL      0       /* raw value as unsigned Integer                        (always Intel Byte order)    */
#define MDF4_CN_VAL_UNSIGN_MOTOROLA   1       /* raw value as unsigned Integer                        (always Motorola Byte order) */
#define MDF4_CN_VAL_SIGNED_INTEL      2       /* raw value as signed Integer (two's complement)       (always Intel Byte order)    */
#define MDF4_CN_VAL_SIGNED_MOTOROLA   3       /* raw value as signed Integer (two's complement)       (always Motorola Byte order) */
#define MDF4_CN_VAL_REAL_INTEL        4       /* raw value as IEEE floating point 2/4/8 Byte          (always Intel Byte order)    */
#define MDF4_CN_VAL_REAL_MOTOROLA     5       /* raw value as IEEE floating point 2/4/8 Byte          (always Motorola Byte order) */

#define MDF4_CN_VAL_STRING_SBC        6       /* String, SBC, US-ASCII (ISO-8859-1), zero terminated                               */
#define MDF4_CN_VAL_STRING_UTF8       7       /* String, UTF-8,  zero terminated                                                   */
#define MDF4_CN_VAL_STRING_UTF16_LE   8       /* String, UTF-16, zero terminated, Intel Byte order                                 */
#define MDF4_CN_VAL_STRING_UTF16_BE   9       /* String, UTF-16, zero terminated, Motorola Byte order                              */
#define MDF4_CN_VAL_BYTE_ARRAY       10       /* Sample is Byte array with unknown content (e.g. structure)                        */
#define MDF4_CN_VAL_MIME_SAMPLE      11       /* Sample is Byte array with MIME content-type specified in cn_md_unit               */
#define MDF4_CN_VAL_MIME_STREAM      12       /* All samples combined are a stream with MIME content-type specified in cn_md_unit  */
#define MDF4_CN_VAL_CO_DATE_STRUCT   13       /* bit structure with CANopen date information          (always 7 Byte)              */
                                              /* according to data type "Date" of the CiA-CANopen-Specification "Application Layer
                                                 and Communication Profile", Version 4.0.                                          */
#define MDF4_CN_VAL_CO_TIME_STRUCT   14       /* bit structure with CANopen time information          (always 6 Byte)              */
                                              /* according to data type "Time" of the CiA-CANopen-Specification "Application Layer
                                                 and Communication Profile", Version 4.0.                                          */
#define MDF4_CN_VAL_COMPLEX_INTEL    15       /* complex number, re part followed by im raw with                                   */
                                              /* two values as IEEE floating point 2/4/8 Byte  (always Intel Byte order)           */
#define MDF4_CN_VAL_COMPLEX_MOTOROLA 16       /* complex number, re part followed by im raw with                                   */
                                              /* two values as IEEE floating point 2/4/8 Byte  (always Motorola Byte order)        */



/* bit structure for MDF4_CN_VAL_CO_DATE_STRUCT */
struct cn_byte_array_date
{
  WORD ms:              16;     /* Bit 0  - Bit 15: Milliseconds  (0 ... 59999)                           */

  BYTE min:              6;     /* Bit 0  - Bit  5: Minutes       (0 ... 59)                              */
  BYTE min_reserved:     2;     /* Bit 7  - Bit  8: reserved                                              */

  BYTE hour:             5;     /* Bit 0  - Bit  4: Hours         (0 ... 23)                              */
  BYTE hour_reserved:    2;     /* Bit 5  - Bit  6: reserved                                              */
  BYTE summer_time:      1;     /* Bit 7          : 0 = Standard time, 1 = Summer time                    */

  BYTE day:              5;     /* Bit 0  - Bit  4: Day           (1 ... 31)                              */
  BYTE week_day:         3;     /* Bit 5  - Bit  7: Week day      (1 = Monday ... 7 = Sunday)             */

  BYTE month:            6;     /* Bit 0  - Bit  5: Month         (1 = January ... 12 = December)         */
  BYTE month_reserved:   2;     /* Bit 6  - Bit  7: reserved                                              */

  BYTE year:             7;     /* Bit 0  - Bit  6: Year          (0 ... 99)                              */
  BYTE year_reserved:    1;     /* Bit 7          : reserved                                              */
};

/* bit structure for MDF4_CN_VAL_CO_TIME_STRUCT */
struct cn_byte_array_time
{
  DWORD ms:             28;     /* Bit 0  - Bit 27: Number of milliseconds since midnight                 */
  DWORD ms_reserved:     4;     /* Bit 28 - Bit 31: reserved                                              */

  WORD days:            16;     /* Bit 0  - Bit 15: Number of days since 1.1.1984 (optional, can be 0)    */
};


#define MDF4_CN_FLAG_ALL_INVALID        (1<<0)   /* all values of this channel are invalid (even if no invalidation bit used) */
#define MDF4_CN_FLAG_INVAL_BIT          (1<<1)   /* invalidation bit is used for this channel
                                                    (must not be set if cg_inval_bytes == 0) */
#define MDF4_CN_FLAG_PRECISION          (1<<2)   /* cn_precision is valid and overrules cc_precision */
#define MDF4_CN_FLAG_VAL_RANGE_OK       (1<<3)   /* raw value range for signal is valid if this bit is set in cn_flags */
#define MDF4_CN_FLAG_VAL_LIMIT_OK       (1<<4)   /* limit range for signal is valid if this bit is set in cn_flags */
#define MDF4_CN_FLAG_VAL_LIMIT_EXT_OK   (1<<5)   /* extended limit range for signal is valid if this bit is set in cn_flags */
#define MDF4_CN_FLAG_DISCRETE_VALUES    (1<<6)   /* values for this channel are discrete and must not be interpolated (see ASAP2 V1.6 keyword DISCRETE) */
#define MDF4_CN_FLAG_CALIBRATION        (1<<7)   /* values for this channel correspond to a calibration object instead to a measurement object
                                                   (see ASAP2 keywords MEASUREMENT and CHARACTERISTIC) */
#define MDF4_CN_FLAG_CALCULATED         (1<<8)   /* values for this channel have been calculated from other channel inputs
                                                   (see ASAP2 keywords VIRTUAL and DEPENDENT_CHARACTERISTIC) */
#define MDF4_CN_FLAG_VIRTUAL            (1<<9)   /* channel is virtual, i.e. it is simulated by the tool
                                                   (see ASAP2 keywords VIRTUAL and VIRTUAL_CHARACTERISTIC) */
#define MDF4_CN_FLAG_BUS_EVENT          (1<<10)  /* channel contains bus event information for bus message logging (valid since MDF 4.1).
                                                    See association standard on bus logging */
#define MDF4_CN_FLAG_MONOTONOUS         (1<<11)  /* channel contains only strictly monotonous increasing/decreasing values (valid since MDF 4.1)*/
#define MDF4_CN_FLAG_DEFAULT_X          (1<<12)  /* channel contains reference to some other channel to be used as X signal in cn_default_x (valid since MDF 4.1) */
#define MDF4_CN_FLAG_EVENT_SIGNAL       (1<<13)  /* channel is used for description of events in an event signal group (valid since MDF 4.2) */
#define MDF4_CN_FLAG_VLSD_DATA_STREAM   (1<<14)  /* SDBLOCK referenced by cn_data contains a stream of VLSD values, i.e. no gaps and correct sorting (valid since MDF 4.2).
                                                    Can only be set for VLSD channel (cn_type = 1) in sorted data group.*/

typedef struct cnblock_links
{
  LINK64       cn_cn_next;                  /* pointer to next CNBLOCK of group (can be NIL) */
  LINK64       cn_composition;              /* pointer to CABLOCK/CNBLOCK to describe components of this signal (can be NIL) */
  LINK64       cn_tx_name;                  /* pointer to TXBLOCK for name of this signal (can be NIL, must not be NIL for data channels)
                                               The name must be unique within the channels of this channel group.
                                               Together with the sender, network and acquisition name defined in parent CGBLOCK,
                                               this combination must be unique within the MDF file.
                                               Note: Alternative names (e.g. display name) can be stored in MDBLOCK of cn_md_comment
                                            */
  LINK64       cn_si_source;                /* pointer to SIBLOCK for name of this signal (can be NIL, must not be NIL for data channels)
                                               The name must be unique within the channels of this channel group.
                                               Together with the sender, network and acquisition name defined in parent CGBLOCK,
                                               this combination must be unique within the MDF file.
                                               Note: Alternative names (e.g. display name) can be stored in MDBLOCK of cn_md_comment
                                            */
  LINK64       cn_cc_conversion;            /* pointer to conversion rule (CCBLOCK) of this signal (can be NIL) */
  LINK64       cn_data;                     /* pointer to DLBLOCK/HLBLOCK/SDBLOCK/DZBLOCK/ATBLOCK/CGBLOCK defining the signal data for this signal (can be NIL) */
                                            /* for channel type MDF4_CN_TYPE_VALUE          : must be NIL
                                               for channel type MDF4_CN_TYPE_MASTER         : must be NIL
                                               for channel type MDF4_CN_TYPE_VIRTUAL_MASTER : must be NIL
                                               for channel type MDF4_CN_TYPE_STREAM_SYNC    : pointer to ATBLOCK (containing a stream with time info)
                                               for channel type MDF4_CN_TYPE_VLSD           : pointer to SDBLOCK (or respective DZBLOCK) or DLBLOCK/HLBLOCK or CGBLOCK (can be NIL)
                                               for event signal struct                      : pointer to EVBLOCK for event template of event signal structure (cannot be NIL)
                                            */
  LINK64       cn_md_unit;                  /* pointer to MD/TXBLOCK with string for physical unit after conversion (can be NIL).
                                               If NIL, the unit from the conversion rule applies. If empty string, no unit should be displayed.
                                               A MDBLOCK can be used to additionally store the ASAM Harmonized Object unit definition
                                               which may use an XML reference to ASAM-HO PHYSICAL-DIMENSION globally defined in MDBLOCK of hd_md_comment.
                                               For master channels (MDF4_CN_TYPE_MASTER_xxx) and MDF4_CN_TYPE_TIME_SYNC type channels,
                                               the ASAM-HO definition should not be used to avoid redundancy.
                                            */
                                            /* XML schema for contents of MDBLOCK see cn_unit.xsd */
  LINK64       cn_md_comment;               /* pointer to TXBLOCK/MDBLOCK with comment/description of this signal and other information (can be NIL) */
                                            /* XML schema for contents of MDBLOCK see cn_comment.xsd */
/* LINK64       cn_at_reference[1];  */     /* list with references to ATBLOCKs, size specified in cn_attachment_count (since MDF 4.1) */
/* CHANNEL64    cn_default_x;        */     /* reference to channel to be used as X axis for this channel (since MDF 4.1).
                                               channel reference consists of 3 links to CN/CG/DG. None of them should be NIL.

                                               Attention: does only exist if MDF4_CN_FLAG_DEFAULT_X (bit 12) is set in cn_flags.
                                            */

} CNBLOCK_LINKS;



typedef struct cnblock_data
{
  BYTE         cn_type;                 /* 1*/  /* channel type [MDF4_CN_TYPE_xxx]                              */
  BYTE         cn_sync_type;            /* 1*/  /* sync type [MDF4_CN_SYNC_TYPE_xxx]                            */
  BYTE         cn_data_type;            /* 1*/  /* data type of value [MDF4_CN_VAL_xxx]                         */
  BYTE         cn_bit_offset;           /* 1*/  /* data bit offset for first bit of signal value [0...7]        */
  DWORD        cn_byte_offset;          /* 4*/  /* data byte offset for first bit of signal value               */
  DWORD        cn_bit_count;            /* 4*/  /* Number of bits for the value in record                       */
                                                /*(also relevant for MDF4_CN_VAL_FLOAT / MDF4_CN_VAL_DOUBLE!)   */
  DWORD        cn_flags;                /* 4*/  /* flags are bit combination of [MDF4_CN_FLAG_xxx] */
  DWORD        cn_inval_bit_pos;        /* 4*/  /* position of invalidation bit (starting after cg_data_bytes!)    */
                                                /* cn_inval_bit_pos & 0x07 gives the bit number [0...7] for        */
                                                /* invalidation bit flag (optional)                                         */
                                                /* cn_inval_bit_pos addresses a single bit                                    */
                                                /* in the record that signals the validity of the value in the record:      */
                                                /* bit == 0: value is valid                                                 */
                                                /* bit == 1: value is invalid                                               */
                                                /*  NOTE: The master channels must not have any invalid values.             */
                                                /*        An invalidation bit defined for a master channel should be ignored! */
  BYTE         cn_precision;            /* 1*/  /* precision of value to use for display (for floating-point values)        */
                                                /* decimal places to use for display of float value (0xFF => infinite)      */
  BYTE         cn_reserved;             /* 1*/  /* reserved */
  WORD         cn_attachment_count;     /* 2*/  /* number of attachments related to this channel, i.e. size of cn_at_references. Can be 0. (since MDF 4.1) */
  REAL         cn_val_range_min;        /* 8*/  /* minimum value of raw value range */
  REAL         cn_val_range_max;        /* 8*/  /* maximum value of raw value range */
  REAL         cn_limit_min;            /* 8*/  /* minimum phys value of limit range */
  REAL         cn_limit_max;            /* 8*/  /* maximum phys value of limit range */
  REAL         cn_limit_ext_min;        /* 8*/  /* minimum phys value of extended limit range */
  REAL         cn_limit_ext_max;        /* 8*/  /* maximum phys value of extended limit range */

} CNBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* CCBLOCK                                                                  */
/* Conversion block                                                         */
/* Information about the conversion from raw to physical value of a channel */
/* Length of this block is variable and depends on the number of parameters */
/* or the length of the table                                               */

#define MDF4_CC_ID              GENERATE_ID('C', 'C')   /* Identification */
#define MDF4_CC_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 7*8)
#define MDF4_CC_MIN_LINK_COUNT  4

#define MDF4_CC_LENGTH(link_count, para_count)  (MDF4_CC_MIN_LENGTH + (link_count)*sizeof(LINK64) + (para_count)*sizeof(REAL))

//                                                          (links),(para)
#define MDF4_CC_LENGTH_NON                   MDF4_CC_LENGTH(0      , 0)        // npar = 0
#define MDF4_CC_LENGTH_LIN                   MDF4_CC_LENGTH(0      , 2)        // npar = 2
#define MDF4_CC_LENGTH_RAT                   MDF4_CC_LENGTH(0      , 6)        // npar = 6
#define MDF4_CC_LENGTH_ALG                   MDF4_CC_LENGTH(1      , 0)        // npar = 1
#define MDF4_CC_LENGTH_TABI(n)               MDF4_CC_LENGTH(0      , (n)*2)    // npar = n
#define MDF4_CC_LENGTH_TAB(n)                MDF4_CC_LENGTH(0      , (n)*2)    // npar = n
#define MDF4_CC_LENGTH_RTAB(n)               MDF4_CC_LENGTH(0      , (n)*3+1)  // npar = n
#define MDF4_CC_LENGTH_TABX(n)               MDF4_CC_LENGTH((n)+1  , (n))      // npar = n
#define MDF4_CC_LENGTH_RTABX(n)              MDF4_CC_LENGTH((n)+1  , (n)*2)    // npar = n
#define MDF4_CC_LENGTH_TTAB(n)               MDF4_CC_LENGTH((n)    , (n)+1)    // npar = n
#define MDF4_CC_LENGTH_TRANS(n)              MDF4_CC_LENGTH((n)*2+1, 0)        // npar = n
#define MDF4_CC_LENGTH_BFIELD(n)             MDF4_CC_LENGTH(n      , n)        // npar = n

#define MDF4_CC_LINK_COUNT_NON               (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_LIN               (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_RAT               (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_ALG               (MDF4_CC_MIN_LINK_COUNT + 1)
#define MDF4_CC_LINK_COUNT_TABI(n)           (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_TAB(n)            (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_RTAB(n)           (MDF4_CC_MIN_LINK_COUNT + 0)
#define MDF4_CC_LINK_COUNT_TABX(n)           (MDF4_CC_MIN_LINK_COUNT + (n)+1)
#define MDF4_CC_LINK_COUNT_RTABX(n)          (MDF4_CC_MIN_LINK_COUNT + (n)+1)
#define MDF4_CC_LINK_COUNT_TTAB(n)           (MDF4_CC_MIN_LINK_COUNT + n)
#define MDF4_CC_LINK_COUNT_TRANS(n)          (MDF4_CC_MIN_LINK_COUNT + (n)*2+1)
#define MDF4_CC_LINK_COUNT_BFIELD(n)         (MDF4_CC_MIN_LINK_COUNT + n)


#define MDF4_CC_FRM_NON     0                /* 1:1 conversion          phy = int */

#define MDF4_CC_FRM_LIN     1                /* linear conversion       phy = p0 + p1 * int */
                                             /* with parameters offset p0 = cc_val[0], factor p1 = cc_val[1] */
                                             /* size(cc_val) = cc_npar = 2;   size(cc_ref) = 0; */
                                             /* Note: p1 can be 0 to implement a constant value */

#define MDF4_CC_FRM_RAT     2                /* rational conversion formula with 6 parameters */
                                             /* phy = (p0*int*int + p1*int + p2) / (p3*int*int + p4*int + p5) with pi =  cc_val[i] */
                                             /* size(cc_val) = cc_npar = 6;   size(cc_ref) = 0; */

#define MDF4_CC_FRM_ALG     3                /* algebraic conversion formula (text) according to ASAM MCD-2MC V1.6 keyword FORMULA */
                                             /* (i.e. subset of ASAM General Expression Syntax V1.0) */
                                             /* cc_ref[0] contains link to TXBLOCK or MDBLOCK with formula */
                                             /* size(cc_val) = 0;   size(cc_ref) = cc_npar = 1; */
                                             /* MDBLOCK may be used to specify an alternative syntax in tag <custom_syntax> */
                                             /* <TX> tag must contain the GES syntax and may specify the GES version in
                                                optional attribute "ges_version"; default value = "1.0" */

#define MDF4_CC_FRM_TABI    4                /* look-up table with interpolation */
                                             /* size(cc_val) = 2*cc_npar;   size(cc_ref) = 0; */
                                             /* key[i] = cc_val[2*i], value[i] = cc_val[2*i+1], for i = {0...(cc_npar-1)} */
                                             /*   Key values must be sorted (strictly monotonously increasing)
                                                  if input_value lies between two key values, linear interpolation is used.
                                                  if input_value <= key[0]          then value[0]         is used
                                                  if input_value >= key[cc_npar-1], then value[cc_npar-1] is used
                                             */

#define MDF4_CC_FRM_TAB     5                /* look-up table without interpolation */
                                             /* size(cc_val) = 2*cc_npar;   size(cc_ref) = 0; */
                                             /* key[i] = cc_val[2*i], value[i] = cc_val[2*i+1], for i = {0...(cc_npar-1)} */
                                             /*   Key values must be sorted (strictly monotonously increasing)
                                                  If input_value does not exactly match a key value, the nearest key value is used.
                                                  If input_value is exactly between two key values the nearest lower key value is used.
                                             */

#define MDF4_CC_FRM_RTAB    6                /* range look-up table [value, value] -> value */                                    /* cc_frm.tabr */
                                             /* size(cc_val) = 3*cc_npar+1;   size(cc_ref) = 0; */
                                             /* lower_range[i] = cc_val[3*i], upper_range[i] = cc_val[3*i+1], value[i] = cc_val[3*i+2], for i = {0...(cc_npar-1)}
                                                default_value = cc_val[3*cc_npar]
                                             */
                                             /* no overlapping ranges must be defined */
                                             /* for float input_value:   if lower_range[i] <= input_value <  lower_range[i], then value[i] is used. */
                                             /* for integer input_value: if lower_range[i] <= input_value <= lower_range[i], then value[i] is used. */
                                             /* if no range fits, then the default_value is used */

#define MDF4_CC_FRM_TABX    7                /* look-up table for text / scale conversion */
                                             /* size(cc_val) = cc_npar;   size(cc_ref) = cc_npar+1; */
                                             /* key[i] = cc_val[i], value_link[i] = cc_ref[i], for i = {0...(cc_npar-1)}
                                                default_value_link = cc_ref[cc_npar] */
                                             /*   Key values must be sorted (strictly monotonously increasing)
                                                  If input_value does not exactly match a key value, the default_value_link is used.
                                             */
                                             /* the value_link can point to
                                                - a TXBLOCK with the description for the input_value
                                                - a CCBLOCK with a conversion to be applied to input_value
                                                - NIL in which case the output value is undefined (not a valid value)
                                                  (note that NIL here does not mean 1:1 conversion!)
                                             */

#define MDF4_CC_FRM_RTABX   8                /* range look-up table for text / scale conversion */
                                             /* size(cc_val) = 2*cc_npar;   size(cc_ref) = cc_npar+1; */
                                             /* lower_range[i] = cc_val[2*i], upper_range[i] = cc_val[2*i+1], value_link[i] = cc_ref[i], for i = {0...(cc_npar-1)}
                                                default_value_link = cc_ref[cc_npar] */
                                             /* no overlapping ranges must be defined */
                                             /* for float input_value:   if lower_range[i] <= input_value <  lower_range[i], then value_link[i] is used. */
                                             /* for integer input_value: if lower_range[i] <= input_value <= lower_range[i], then value_link[i] is used. */
                                             /* if no range fits, then the default_value_link is used */
                                             /* the value_link can point to
                                                - a TXBLOCK with the description for the input_value
                                                - a CCBLOCK with a conversion to be applied to input_value
                                                - NIL in which case the output value is undefined (not a valid value)
                                                  (note that NIL here does not mean 1:1 conversion!)
                                             */

#define MDF4_CC_FRM_TTAB     9               /* text to value look-up table */
                                             /* size(cc_val) = cc_npar + 1;   size(cc_ref) = cc_npar; */
                                             /* key[i] = cc_ref[i], value[i] = cc_val[i], for i = {0...(cc_npar-1)}
                                                default_value = cc_val[cc_npar] */
                                             /* if the input text matches (case sensitive string compare) the text referenced by key[i], then value[i] is used */
                                             /* if no key matches, the default_value is used */


#define MDF4_CC_FRM_TRANS   10               /* text to text look-up table (translation) */
                                             /* size(cc_val) = 0;   size(cc_ref) = 2*cc_npar+1; */
                                             /* key[i] = cc_ref[2*i], value_link[i] = cc_ref[2*i+1], for i = {0...(cc_npar-1)}
                                                default_value_link = cc_ref[cc_npar] */
                                             /* if the input text matches (case sensitive string compare) the text referenced by key[i], then value_link[i] is used */
                                             /* if no key matches, the default_value_link is used */
                                             /* the value_link can point to
                                                - a TXBLOCK with the translation for the input text
                                                - NIL in which case the input text is used (1:1 translation)
                                             */

#define MDF4_CC_FRM_BITFIELD_TAB  11         /* value to string conversion using a table with partial bit mask to text conversion */
                                             /* size(cc_val) = cc_npar;   size(cc_ref) = cc_npar; */
                                             /* bitmask[i] = cc_val[i], conversion_link[i] = cc_ref[i], for i = {0...(cc_npar-1)} */
                                             /* input value & bitmask[i] is the input value for the respective conversion (TABX or RTABX) which delivers a string or "nothing"*/
                                             /* the result text is assembled by concatenating the strings with | as separator, ignoring "nothing" strings and possibly
											                          preceding each string with "<name> = " if the respective conversion has a name (cc_tx_name) */


#define MDF4_CC_FLAG_PRECISION     (1<<0)    /* cc_precision is valid */
#define MDF4_CC_FLAG_PHY_RANGE_OK  (1<<1)    /* physical value range for signal is valid if this bit is set in cc_flags */
#define MDF4_CC_FLAG_STATUS_STRING (1<<2)    /* indicates that a MDF4_CC_FRM_TABX/MDF4_CC_FRM_RTABX table represents status strings
                                                (only links to TXBLOCK for table entries), the actual conversion rule is given in
                                                CCBLOCK referenced by default value.
                                                This also implies special handling of limits, see ASAP2 V1.6 keyword STATUS_STRING_REF.
                                             */

/*
  look-up tables
  for "normal"  CC rule: input_value = raw  value, output = phys value
  for "inverse" CC rule: input_value = phys value, output = raw value
*/

typedef struct ccblock_links
{
  LINK64       cc_tx_name;                /* pointer to TXBLOCK with identification string of conversion rule (can be NIL) */
  LINK64       cc_md_unit;                /* pointer to TX/MDBLOCK with string for physical unit after conversion (can be NIL)
                                             A MDBLOCK can be used to additionally store the ASAM Harmonized Object unit definition
                                             which may use an XML reference to ASAM-HO PHYSICAL-DIMENSION globally defined in MDBLOCK of hd_md_comment.
                                             For master channels (MDF4_CN_TYPE_MASTER_xxx) and MDF4_CN_TYPE_TIME_SYNC type channels,
                                             the ASAM-HO definition should not be used to avoid redundancy.
                                             only applies if no unit defined in CNBLOCK */
                                          /* XML schema for contents of MDBLOCK see cc_unit.xsd */
  LINK64       cc_md_comment;             /* pointer to TXBLOCK/MDBLOCK with comment and other information for conversion rule (can be NIL) */
                                          /* XML schema for contents of MDBLOCK see cc_comment.xsd */
  LINK64       cc_cc_inverse;             /* pointer to CCBLOCK with inverse conversion rule (can be NIL) */
                                          /* cc_cc_inverse and cc_md_unit for inverse formula must be NIL. */
  // @@@@ LINK64       cc_ref[1];                 
  /* variable length list of links with references to TXBLOCK/MDBLOCK or CCBLOCK.
                                             (MDBLOCK only allowed for algebraic conversion).
                                             The list can be empty.
                                             See explanation of conversion types
                                          */

} CCBLOCK_LINKS;

typedef struct ccblock_data
{
  BYTE         cc_type;                    /* 1*/  /* conversion type identifier [MDF4_CC_FRM_xxx] */
  BYTE         cc_precision;               /* 1*/  /* number of decimal places to use for display of float value
                                                      (0xFF => infinite)
                                                      only valid if MDF4_CC_FLAG_PRECISION flag is set */
  WORD         cc_flags;                   /* 2*/  /* flags are bit combination of [MDF4_CC_FLAG_xxx] */
  WORD         cc_ref_count;               /* 2*/  /* length of cc_ref list */
  WORD         cc_val_count;               /* 2*/  /* length of cc_val list */
  REAL         cc_phy_range_min;           /* 8*/  /* minimum value of physical value range */
  REAL         cc_phy_range_max;           /* 8*/  /* maximum value of physical value range */

  REAL /* or UINT64*/  cc_val[2];          /* 8*/  /* variable length list of parameter values, list can be empty
                                                      see explanation of conversion types */

} CCBLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* CABLOCK                                                                  */
/* Channel Array block                                                      */
/* N-dimensional dependency of a channel from an arbitrary number of others */
/* All channels that are referenced must have the same data type            */
/* The length of this block is variable                                     */
/* Attention: both CABLOCK_LINKS and CABLOCK_DATA have a variable length    */

/* Convention:
   the channel that owns CABLOCK does not have to contain any values,
   but usually it will contain values of matrix element[0,0]
   and the data type that is required for all elements.                    */

#define MDF4_CA_ID              GENERATE_ID('C', 'A')   /* Identification */
#define MDF4_CA_MIN_LENGTH      (sizeof(BLOCK_HEADER) + (3*8))
#define MDF4_CA_MIN_LINK_COUNT  (sizeof(CABLOCK_LINKS) / sizeof(LINK64))

#define MDF4_CA_STORAGE_CN_TEMPLATE       0      /* parent CNBLOCK is used as template for elements.
                                                    Start Byte will be incremented by Start Byte Offset specified in CABLOCK */

#define MDF4_CA_STORAGE_CG_TEMPLATE       1      /* parent CGBLOCK is used as template for elements (unsorted MDF file)
                                                    For each array element, a record ID, the element value and possibly a time stamp
                                                    will be stored in the DTBLOCK(s) or DVBLOCKS referenced by parent DGBLOCK.
                                                    The record IDs will be auto incremented starting with the record ID specified by
                                                    parent CG block.
                                                    The cycle counts for each element will be specified in the ca_cycle_count list of CABLOCK.
                                                    */

#define MDF4_CA_STORAGE_DG_TEMPLATE       2      /* parent CGBLOCK is used as template for elements (sorted MDF file)
                                                    this is the sorted version of MDF4_CA_STORAGE_CG_TEMPLATE
                                                    For each array element, the element value and possibly a time stamp will be stored
                                                    in the data blocks as referenced by the respective ca_data link.
                                                    The cycle counts for each element will be specified in the ca_cycle_count list of CABLOCK.
                                                 */


#define MDF4_CA_TYPE_VAL_ARRAY             0     /* simple array of values, no axis used */
#define MDF4_CA_TYPE_SCALE_AXIS            1     /* only for 1-dimensional vector that is used as scaling axis */
#define MDF4_CA_TYPE_LOOKUP                2     /* look-up, i.e. curve, map or cuboid that used variable or fixed scaling axes */
#define MDF4_CA_TYPE_INTERVAL_AXIS         3     /* array contains axis with intervals (since MDF 4.1) */
#define MDF4_CA_TYPE_CLASSIFICATION_RESULT 4     /* array contains classification results, each axis has one axis point more than the respective dimension of the array (since MDF 4.1) */


#define MDF4_CA_FLAG_DYNAMIC_SIZE        (1<<0)  /* if set, the number of scaling points for the array is not fixed but can vary over time.
                                                    must not be set for ca_type = MDF4_CA_TYPE_VAL_ARRAY */
#define MDF4_CA_FLAG_INPUT_QUANTITY      (1<<1)  /* if set, a channel for the input quantity is specified for each dimension by ca_input_quantity.
                                                    must not be set for ca_type = MDF4_CA_TYPE_VAL_ARRAY */
#define MDF4_CA_FLAG_OUTPUT_QUANTITY     (1<<2)  /* if set, a channel for the output quantity is specified by ca_output_quantity.
                                                    can only be set for ca_type = MDF4_CA_TYPE_LOOKUP */
#define MDF4_CA_FLAG_COMPARISON_QUANTITY (1<<3)  /* if set, a channel for the comparison quantity is specified by ca_comparison_quantity.
                                                    can only be set for ca_type = MDF4_CA_TYPE_LOOKUP */
#define MDF4_CA_FLAG_AXIS                (1<<4)  /* if set, a scaling axis is given for each dimension of the array, either as fixed or as dynamic axis,
                                                    depending on MDF4_CA_FLAG_FIXED_AXIS flag.
                                                    must not be set for ca_type = MDF4_CA_TYPE_VAL_ARRAY */
#define MDF4_CA_FLAG_FIXED_AXIS          (1<<5)  /* the scaling axis is fixed and the axis points are stored as raw values in ca_axis_value
                                                    only relevant if MDF4_CA_FLAG_AXIS flag is set */
#define MDF4_CA_FLAG_INVERSE_LAYOUT      (1<<6)  /* Only relevant for ca_storage == MDF4_CA_STORAGE_CN_TEMPLATE
                                                    If this flag is not set, the record layout is considered "row oriented", i.e.

                                                      n    = number of dimensions = ca_ndim
                                                      d(i) = index of dimension i (i = 1...n)
                                                      D(i) = number of axis points for dimension i = ca_dim_size[i-1] or value of size signal if MDF4_CA_FLAG_DYNAMIC_LAYOUT
                                                      f(1) = ca_byte_offset_base
                                                      f(2) = f(1) x D(1)
                                                      ...
                                                      f(n) = f(n) x D(n)
                                                      B    = Byte offset of previous level (e.g. cn_byte_offset for CN template)
                                                      the Byte offset is calculated by: B + d(1) * f(1) + d(2) * f(2) + ... + d(n) * f(n)

                                                      if the bit is set, the direction is inverted, i.e.
                                                      f(n)   = ca_byte_offset_base
                                                      f(n-1) = f(n) x D(n)
                                                      ...
                                                      f(1)   = f(2) x D(2)

                                                    see ASAP2 V1.6 keywords ROW_DIR and COLUMN_DIR
                                                 */

#define MDF4_CA_FLAG_INTERVAL_LEFT_OPEN  (1<<7)  /* Interval left open, only for ca_type = MDF4_CA_TYPE_INTERVAL_AXIS (since MDF 4.1)*/

#define MDF4_CA_FLAG_STANDARD_AXIS       (1<<8)  /* the scaling axis is a standard axis (since MDF 4.2)
                                                    only relevant if MDF4_CA_FLAG_AXIS flag is set and MDF4_CA_FLAG_FIXED_AXIS is NOT set
													                          can only be set for ca_type = MDF4_CA_TYPE_SCALE_AXIS or MDF4_CA_TYPE_LOOKUP
                                                 */


//#define MDF4_CA_FLAG_DYNAMIC_LAYOUT      (1<<8)
                                                 /* Only relevant for ca_storage == MDF4_CA_STORAGE_CN_TEMPLATE and ca_dynamic_size != NIL
                                                    If this flag is not set, for multi-dimensional look-ups with dynamic number of axis points
                                                    the record layout does not compact or expand data when removing resp. inserting axis points.
                                                    All record layout elements are stored at the same address as for the max. number of axis points
                                                    specified in ca_dim_size - independent of the actual number of axis points.
                                                    If this flag is set, the record layout does compact / extend data when removing resp. inserting axis points
                                                    and the addresses of the record layout elements depend on the actual number of axis points.
                                                    see ASAP2 V1.6 keyword STATIC_RECORD_LAYOUT
                                                   */


typedef struct cablock_links
{
  LINK64       ca_composition;            /* link to CA or CN block for description of components (can be NIL).
                                             link to CA block for array of arrays (referenced CA block must use storage type "CN template")
                                             link to CN block for list of structure members (Byte offset of each member element is relative
                                             to start Byte of complete structure element).
                                             Note: the CN blocks for the elements of the structure cannot be children of a CGBLOCK!
                                          */

/* LINK64    ca_data[1];  */              /* variable length list with links to data blocks (DTBLOCK, DVBLOCK, respective DZBLOCK or data list (DLBLOCK/HLBLOCK/LDBLOCK) for each element in matrix.
                                              the referred DTBLOCK(s) contain(s) all records for this element in the N-dimensional
                                              matrix measured over time. */
                                          /* Attention: list does only exist for storage type "DG template"
                                             (ca_storage = MDF4_CA_STORAGE_DG_TEMPLATE)
                                             length of list must be equal to product of all elements of ca_dimsize:
                                              __
                                              ||  ca_dimsize[i]
                                            i = 0

                                          */

/* CHANNEL64 ca_dynamic_size[1]; */       /* variable length list with reference to channels for size signals for each dimension (can be NIL).
                                             channel reference consist of 3 links to CN/CG/DG. Either all of them are assigned or NIL.
                                             the size signal for a dimension specifies the currently used number of axis points.
                                             The value must not exceed the specified maximum number of elements for the resp. dimension.
                                          */
                                          /*
                                             Attention: does only exist for array types "axis" and "lookup"
                                             (ca_type != MDF4_CA_TYPE_VAL_ARRAY && ca_type != MDF4_CA_TYPE_CLASSIFICATION)
                                              length of list must be equal to ca_ndim
                                          */

/* CHANNEL64 ca_input_quantity[1]; */     /* variable length list with reference to channel for input quantity for each dimension (can be NIL).
                                             Each channel reference consists of 3 links to CN/CG/DG. Either all of them are assigned or NIL.
                                             For ca_type MDF4_CA_TYPE_SCALE_AXIS, the input quantity must be applied to the curve defined by this CABLOCK.
                                             For ca_type  MDF4_CA_TYPE_LOOKUP, the value of the input quantity must be applied to the axis of
                                             the respective dimension to get the index of the current working point.
                                          */
                                          /*
                                             Attention: does only exist for array types "axis" and "lookup"
                                             (ca_type != MDF4_CA_TYPE_VAL_ARRAY && ca_type != MDF4_CA_TYPE_CLASSIFICATION)
                                             length of list must be equal to ca_ndim
                                          */
/* CHANNEL64    ca_output_quantity; */    /* reference to channel for output quantity (can be NIL).
                                             channel reference consists of 3 links to CN/CG/DG. Either all of them are assigned or NIL.
                                          */
                                          /*
                                             Attention: does only exist for array types "axis" and "lookup"
                                             (ca_type != MDF4_CA_TYPE_VAL_ARRAY && ca_type != MDF4_CA_TYPE_CLASSIFICATION)
                                          */
/* CHANNEL64    ca_comparison_quantity;*/ /* reference to channel for comparison quantity (can be NIL).
                                             channel reference consists of 3 links to CN/CG/DG. Either all of them are assigned or NIL.
                                          */
                                          /*
                                             Attention: does only exist for array types "axis" and "lookup"
                                             (ca_type != MDF4_CA_TYPE_VAL_ARRAY && ca_type != MDF4_CA_TYPE_CLASSIFICATION)
                                          */


/* LINK64    ca_cc_axis_conversion[1];*/  /* variable length list with links to CCBLOCK blocks for axes (can be NIL).
                                             If NIL, a 1:1 conversion is assumed.
                                             Note: the CC rules for axis objects will be overruled.
                                             If fixed axis values are stored as raw values, the CC rule must be applied to the raw values.
                                          */
                                          /*
                                             Attention: does only exist for array types "axis" and "lookup" and "classification result"
                                             (ca_type != MDF4_CA_TYPE_VAL_ARRAY)
                                             length of list must be equal to ca_ndim
                                          */

/* CHANNEL64 ca_axis[1]; */               /* variable length list with channel references for non-fixed scaling axes
                                             i.e. a referenced channel contains the axis scaling points measured over time.
                                             Each channel reference consists of 3 links to CN/CG/DG. Cannot be NIL!
                                             Each channel must be array of type "axis" (ca_type = MDF4_CA_TYPE_SCALE_AXIS or MDF4_CA_TYPE_INTERVAL_AXIS) with
                                             at least ca_dimsize[i] elements (ca_dimsize[i]+1 in case of ca_type == MDF4_CA_TYPE_CLASSIFICATION).
                                             This channel contains the axis scaling points measured over time.
                                          */
                                          /* Attention: list does only exist for array types "axis" and "lookup" and "classification result"
                                             if the fixed axis flag (MDF4_CA_FLAG_FIXED_AXIS) is NOT set in ca_flags
                                             length of list must be equal to ca_ndim
                                          */


} CABLOCK_LINKS;

typedef struct cablock_data
{
  BYTE          ca_type;            /*1*/ /* array type     [MDF4_CA_TYPE_xxx]               */
  BYTE          ca_storage;         /*1*/ /* storage type   [MDF4_CA_STORAGE_xxx_TEMPLATE]    */
  WORD          ca_ndim;            /*2*/ /* number of dimensions > 0*/
                                          /*
                                            ca_ndim == 1:  vector dependency ca_element[i] = F(x)     where i = x
                                            ca_ndim == 2:  matrix dependency ca_element[i] = F(x,y)   where i = x + y*ca_dimsize[1]
                                            ca_ndim == 3:  cubic  dependency ca_element[i] = F(x,y,z) where i = x + y*ca_dimsize[1] + z*ca_dimsize[1]*ca_dimsize[2]
                                            ...
                                          */

                                          /* Example for ca_ndim = 2 and the following Matrix of size A x B => ca_dimsize[0] = A, ca_dimsize[1] = B

                                                                    -                            -
                                                                   |  M(0,0)  , ... , M(0,A-1),   |
                                                                   |  M(1,0)  , ... , M(1,A-1),   |
                                              F(x,y) = M(y,x) =    |  .               .           |
                                                                   |  .               .           |
                                                                   |  M(B-1,0), ... , M(B-1,A-1)  |
                                                                    -                            -

                                              => ordering in ca_element list:  M(0,0), ... , M(0,A-1), M(1,0), ... , M(1,A-1), ... , M(B-1,0), ... , M(B-1,A-1)
                                          */

  DWORD         ca_flags;               /*4*/ /* flags are bit combination of [MDF4_CA_FLAG_xxx] */
  LONG          ca_byte_offset_base;    /*4*/ /* used as base to calculate the Byte offset in case of CN template. Can be negative! */
  DWORD         ca_inval_bit_pos_base;  /*4*/ /* used as base to calculate the invalidation bit number in case of CN template */

/* QWORD        ca_dim_size[1];  */ /*8*/ /* variable length list with maximum number of elements for each dimension,
                                             length of list must be equal to ca_ndim. */
/* REAL         ca_axis_value[1];*/ /*8*/ /* variable length list with (fixed) axis points (raw value) for each dimension */
                                          /* length of list must be equal to sum of all elements of ca_dimsize:
                                              __
                                              >_  ca_dimsize[i]
                                             i = 0

                                             Attention: list does only exist if the fixed axis flag (MDF4_CA_FLAG_FIXED_AXIS) is set in ca_flags
                                             (ca_type then should not be equal to MDF4_CA_TYPE_VAL_ARRAY)

                                             The points of an axis are stored in the order of the dimension.
                                             For example, if we assume 3 dimension (X, Y and Z), the axis points are
                                             X(0),X(1),...,X(ca_dimsize[0]-1),Y(0),Y(1),...,Y(ca_dimsize[1]-1),Z(0),Z(1),...,Z(ca_dimsize[2]-1)

                                           */
                                          /* the physical axis point values can be determined by applying the conversion rule (CCBLOCK)
                                             of the respective scaling axis (see ca_cc_axis_conversion in CABLOCK_LINKS).
                                             If a CCBLOCK link is NIL, then ca_axis_value already contains the physical values (raw = phys)
                                             for this scaling axis.
                                          */
/* QWORD       ca_cycle_count[1];*//*8*/ /* variable length list with cycle count for each element.
                                             ca_cycle_count[0] must be equal to cg_cycle_count of parent CGBLOCK!
                                          */
                                          /* Attention: list does only exist for storage types "CG/DG template"
                                             (ca_storage != MDF4_CA_STORAGE_CN_TEMPLATE)
                                             length of list must be equal to product of all elements of ca_dimsize:
                                              __
                                              ||  ca_dimsize[i]
                                            i = 0

                                          */

} CABLOCK_DATA;



/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DTBLOCK                                                                  */
/* data block                                                               */
/* collection of records                                                    */

#define MDF4_DT_ID          GENERATE_ID('D', 'T')   /* Identification */
#define MDF4_DT_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_DT_LINK_COUNT  0

// Data part can be empty!
typedef struct dtblock_data
{
  BYTE         dt_data[1];         /* variable length of data bytes, see extra description of data block */
} DTBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DVBLOCK                                                                  */
/* data value block (since MDF 4.2)                                         */
/* collection of values for COS                                             */

#define MDF4_DV_ID          GENERATE_ID('D', 'V')   /* Identification */
#define MDF4_DV_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_DV_LINK_COUNT  0

// Data part can be empty!
typedef struct dvblock_data
{
  BYTE         dv_data[1];         /* variable length of data bytes, see extra description of data block */
} DVBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DIBLOCK                                                                  */
/* invalidation data block (since MDF 4.2)                                  */
/* collection of invalidation bytes (bits) for COS                          */

#define MDF4_DI_ID          GENERATE_ID('D', 'I')   /* Identification */
#define MDF4_DI_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_DI_LINK_COUNT  0

// Data part can be empty!
typedef struct diblock_data
{
  BYTE         di_data[1];         /* variable length of data bytes, see extra description of data block */
} DIBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* SRBLOCK                                                                  */
/* sample reduction block                                                   */
/* reduced signal data for a channel group, used for display of preview     */

#define MDF4_SR_ID          GENERATE_ID('S', 'R')   /* Identification */
#define MDF4_SR_MIN_LENGTH  (sizeof(BLOCK_HEADER) + 5*8)
#define MDF4_SR_MIN_LINK_COUNT  (sizeof(SRBLOCK_LINKS) / sizeof(LINK64))

#define MDF4_SR_SYNC_TIME             MDF4_SYNC_TIME       /* sr_interval contains time interval in seconds    */
#define MDF4_SR_SYNC_ANGLE            MDF4_SYNC_ANGLE      /* sr_interval contains angle interval in radians   */
#define MDF4_SR_SYNC_DISTANCE         MDF4_SYNC_DISTANCE   /* sr_interval contains distance interval in meters */
#define MDF4_SR_SYNC_INDEX            MDF4_SYNC_INDEX      /* sr_interval contains index interval for zero-based record index */

#define MDF4_SR_FLAG_INVAL_BYTES         (1<<0)   /* sample reduction record contains invalidation Bytes. Must only be set if cg_inval_bytes > 0. */
#define MDF4_SR_FLAG_DOMINANT_INVAL_BIT  (1<<1)   /* If set, the invalidation bit for the sample reduction record must be set when any of the underlying raw records is invalid, otherwise if all are invalid (valid since MDF 4.2, can only be set if Bit 0 is set). */

typedef struct srblock_links
{
  LINK64       sr_sr_next;                 /* pointer to next sample reduction block (SRBLOCK) (can be NIL)  */
  LINK64       sr_data;                    /* pointer to reduction data block (RDBLOCK, RVBLOCK or respective DZBLOCK) or data list block (DLBLOCK/HLBLOCK/LDBLOCK) with sample reduction records */
} SRBLOCK_LINKS;

typedef struct srblock_data
{
  QWORD        sr_cycle_count;     /*8*/   /* Number of cycles, i.e. number of sample reduction records in RDBLOCK */
  REAL         sr_interval;        /*8*/   /* Constant length for sample interval > 0, unit depending on sr_sync_type */
                                           /* interval(i) = [i*sr_interval, (i+1)*sr_interval[  for i = 0,...
                                              relative to matching start value in HDBLOCK,
                                              or for MDF4_SR_SYNC_INDEX to record index 0 in parent channel group
                                           */

  BYTE         sr_sync_type;       /*1*/   /* sync type  [MDF4_SR_SYNC_xxx] */
                                           /* only sync types for which a corresponding master channel occurs in parent channel group
                                              or MDF4_SR_SYNC_INDEX allowed.
                                              For MDF4_SR_SYNC_INDEX, the record in the channel group is specified by the index value,
                                              sr_length is the number of indices combined to one value (sr_interval should be integer value > 1)
                                           */
  BYTE         sr_flags;           /*1*/   /* flags are bit combination of [MDF4_SR_FLAG_xxx] */
  BYTE         sr_reserved[6];     /*6*/   /* reserved */
} SRBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* RDBLOCK                                                                  */
/* reduction data block                                                     */
/* collection of sample reduction records                                   */
/* (triples of normal record optionally including invalidation Bytes)       */

#define MDF4_RD_ID          GENERATE_ID('R', 'D')   /* Identification */
#define MDF4_RD_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_RD_LINK_COUNT  0

// Data part can be empty!
typedef struct rdblock_data
{
  BYTE         rd_data[1];         /* variable length of data bytes, see extra description of reduction data */
} RDBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* RVBLOCK                                                                  */
/* reduction data value block (since MDF 4.2)                               */
/* collection of sample reduction values                                    */
/* (triples of normal value without invalidation Bytes)                     */

#define MDF4_RV_ID          GENERATE_ID('R', 'V')   /* Identification */
#define MDF4_RV_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_RV_LINK_COUNT  0

// Data part can be empty!
typedef struct rvblock_data
{
  BYTE         rv_data[1];         /* variable length of data bytes, see extra description of reduction data */
} RVBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* RIBLOCK                                                                  */
/* reduction data invalidation block (since MDF 4.2)                        */
/* collection of sample reduction invalidation bytes (bits) for COS         */
/* (single invalidation Byte areas)                                         */

#define MDF4_RI_ID          GENERATE_ID('R', 'I')   /* Identification */
#define MDF4_RI_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_RI_LINK_COUNT  0

// Data part can be empty!
typedef struct riblock_data
{
  BYTE         ri_data[1];         /* variable length of data bytes, see extra description of reduction data */
} RIBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* SDBLOCK                                                                  */
/* signal data block                                                        */
/* stores signal values with variable length                                */

#define MDF4_SD_ID          GENERATE_ID('S', 'D')   /* Identification */
#define MDF4_SD_MIN_LENGTH  (sizeof(BLOCK_HEADER))
#define MDF4_SD_LINK_COUNT  0

// Data part can be empty!
typedef struct sdblock_data
{
  BYTE         sd_data[1];         /* variable length of data bytes, see extra description of data block */
} SDBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DLBLOCK                                                                  */
/* data list block                                                          */
/* collection of DT or SD or RD blocks                                      */

#define MDF4_DL_ID              GENERATE_ID('D', 'L')   /* Identification */
#define MDF4_DL_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 3*8)
#define MDF4_DL_MIN_LINK_COUNT  (sizeof(DLBLOCK_LINKS)/sizeof(LINK64)-1)


#define MDF4_DL_FLAG_EQUAL_LENGTH  (1<<0)  /* If set, each DLBLOCK in the linked list has the same number of referenced blocks (dl_count)
                                              and the data section lengths of each referenced block are equal and the length is given by dl_equal_length.
                                              The only exception is that for the last DLBLOCK in the list (dl_dl_next = NIL),
                                              its number of referenced blocks dl_count can be less than or equal to dl_count of the previous DLBLOCK,
                                              and the data section length of its last referenced block (dl_data[dl_count-1]) can be less than or equal to dl_equal_length.

                                              If not set, the number of referenced blocks dl_count may be different for each DLBLOCK in the linked list,
                                              and the data section lengths of the referenced blocks may be different
                                              and a table of offsets is given in dl_offset.

                                              Note: The value of MDF4_DL_FLAG_EQUAL_LENGTH must be equal for all DLBLOCKs in the linked list.
                                              */

#define MDF4_DL_FLAG_TIME_VALUES  (1<<1)   /* If set, the DLBLOCK contains a list of (raw) time values in dl_time_values which can be used to improve performance for binary search of time values in the list of referenced data blocks.

                                              The bit must not be set if the DL block references signal data blocks or
                                              if the DL block contains records from multiple channel groups (i.e. if parent is an unsorted data group).
                                              The bit can only be set if the channel group which defines the record layout contains either a virtual or
                                              a non-virtual time master channel (cn_type = 2 or 3 and cn_sync_type = 1).

                                              Note that for RD blocks, it only makes sense to store the time values
                                              if the parent SR block has sr_sync_type = 1 (time).

                                              Note: The value of the "time values" flag must be equal for all DLBLOCKs in the linked list.

                                              Valid since MDF 4.2.0, should not be set for earlier versions
                                              */

#define MDF4_DL_FLAG_ANGLE_VALUES  (1<<2)  /* If set, the DLBLOCK contains a list of (raw) angle values in dl_angle_values which can be used to improve performance for binary search of angle values in the list of referenced data blocks.

                                              The bit must not be set if the DL block references signal data blocks or
                                              if the DL block contains records from multiple channel groups (i.e. if parent is an unsorted data group).
                                              The bit can only be set if the channel group which defines the record layout contains either a virtual or
                                              a non-virtual angle master channel (cn_type = 2 or 3 and cn_sync_type = 2).

                                              Note that for RD blocks, it only makes sense to store the angle values
                                              if the parent SR block has sr_sync_type = 2 (angle).

                                              Note: The value of the "angle values" flag must be equal for all DLBLOCKs in the linked list.

                                              Valid since MDF 4.2.0, should not be set for earlier versions
                                              */

#define MDF4_DL_FLAG_DISTANCE_VALUES (1<<3) /* If set, the DLBLOCK contains a list of (raw) distance values in dl_distance_values which can be used to improve performance for binary search of distance values in the list of referenced data blocks.

                                              The bit must not be set if the DL block references signal data blocks or
                                              if the DL block contains records from multiple channel groups (i.e. if parent is an unsorted data group).
                                              The bit can only be set if the channel group which defines the record layout contains either a virtual or
                                              a non-virtual distance master channel (cn_type = 2 or 3 and cn_sync_type = 3).

                                              Note that for RD blocks, it only makes sense to store the distance values
                                              if the parent SR block has sr_sync_type = 3 (distance).

                                              Note: The value of the "distance values" flag must be equal for all DLBLOCKs in the linked list.

                                              Valid since MDF 4.2.0, should not be set for earlier versions
                                              */


typedef struct dlblock_links
{
  LINK64       dl_dl_next;           /* pointer to (next) data list block (DLBLOCK) (can be NIL)
                                     */
  LINK64       dl_data[1];           /* variable length list of links to DTBLOCKs or SDBLOCKs or RDBLOCKs (mix not allowed!).
                                        Note: each of the blocks may be replaced by a DZBLOCK for the respective block type.
                                        length of list must be equal to dl_count
                                     */
} DLBLOCK_LINKS;


typedef struct dlblock_data
{
  BYTE         dl_flags;             /*1*/  /* flags are bit combination of [MDF4_DL_FLAG_xxx] */
  BYTE         dl_reserved[3];       /*3*/  /* reserved */
  DWORD        dl_count;             /*4*/  /* number of referenced data blocks

                                              If the MDF4_DL_FLAG_EQUAL_LENGTH is set, then dl_count must be equal for each DLBLOCK in the linked list
                                              except for the last one. For the last DLBLOCK (i.e. dl_dl_next = NIL) in this case the value of dl_count
                                              can be less than or equal to dl_count of the previous DLBLOCK.
                                            */
  QWORD        dl_equal_length;      /*8*/ /* Every block in dl_data list has a data section with a length equal to dl_equal_length.
                                              This must be true for each DLBLOCK within the linked list, and has only one exception:
                                              the very last block (dl_data[dl_count-1] of last DLBLOCK in linked list) may have a data section
                                              with a different length which must be less than or equal to dl_equal_length.

                                              only present if MDF4_DL_FLAG_EQUAL_LENGTH is set in dl_flags
                                            */

 /* QWORD        dl_offset[1]; */    /*8*/  /* variable length list of offset values in Bytes. Length of list must be equal to dl_count.
                                              only present if MDF4_DL_FLAG_EQUAL_LENGTH is NOT set in dl_flags

                                              dl_offset[i] is the accumulated data offset for the referenced block that starts at dl_data[i].
                                              If all BLOCK_DATA parts of the blocks in dl_data list of the previous DLBLOCK and of the current DLBLOCK are concatenated,
                                              dl_offset[i] thus is the offset where the BLOCK_DATA parts of the block dl_data[i] would start.

                                                            n - 1
                                                             __
                                              dl_offset[n] = >_  (dl_data[i].length - sizeof(BLOCK_HEADER) - dl_data[i].link_count * sizeof(LINK64))

                                                            i = 0

                                              (i is the running index over all referenced blocks of all DLBLOCKs in the linked list up to the current DLBLOCK)

                                              Hence, dl_offset[0] of the very first DLBLOCK in the linked list must always be zero.
                                            */
  /* INT64/REAL    dl_time_values[1]; */ /*8*/  /* variable length list of raw time values (since MDF 4.2)
                                               Length of list must be equal to dl_count.
                                             only present if MDF4_DL_FLAG_TIME_VALUES is set in dl_flags

                                             dl_time_values[i] is the (raw) time value of the first record in the respective data block.
                                             The value must be interpreted either as INT64 or REAL, depending on data type of time master channel.
                                             In case of RD blocks, the interval start time is used, i.e. the (raw) time value in sub record 1 (see Table 61).
                                             Note that there is no use case for dl_time_values in case the DL block references signal data blocks (SD block or respective DZ block).
                                             */

  /* INT64/REAL     dl_angle_values[1]; */ /*8*/  /* variable length list of raw angle values (since MDF 4.2)
                                               Length of list must be equal to dl_count.
                                               only present if MDF4_DL_FLAG_ANGLE_VALUES is set in dl_flags

                                               dl_angle_values[i] is the (raw) angle value of the first record in the respective data block.
                                               The value must be interpreted either as INT64 or REAL, depending on data type of angle master channel.
                                               In case of RD blocks, the interval start angle is used, i.e. the (raw) angle value in sub record 1 (see Table 61).
                                               Note that there is no use case for dl_angle_values in case the DL block references signal data blocks (SD block or respective DZ block).
                                               */

  /* INT64/REAL     dl_distance_values[1]; */ /*8*/  /* variable length list of raw distance values (since MDF 4.2)
                                                  Length of list must be equal to dl_count.
                                                  only present if MDF4_DL_FLAG_DISTANCE_VALUES is set in dl_flags

                                                  dl_distance_values[i] is the (raw) distance value of the first record in the respective data block.
                                                  The value must be interpreted either as INT64 or REAL, depending on data type of distance master channel.
                                                  In case of RD blocks, the interval start distance is used, i.e. the (raw) distance value in sub record 1 (see Table 61).
                                                  Note that there is no use case for dl_distance_values in case the DL block references signal data blocks (SD block or respective DZ block).
                                                  */

} DLBLOCK_DATA;

/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* LDBLOCK                                                                  */
/* list data block (since MDF 4.2)                                          */
/* collection of DV or RV (and DI or RI) blocks                                            */

#define MDF4_LD_ID              GENERATE_ID('L', 'D')   /* Identification */
#define MDF4_LD_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 3*8)
#define MDF4_LD_MIN_LINK_COUNT  (sizeof(LDBLOCK_LINKS)/sizeof(LINK64)-1)


#define MDF4_LD_FLAG_EQUAL_SAMPLE_COUNT MDF4_DL_FLAG_EQUAL_LENGTH
#define MDF4_LD_FLAG_TIME_VALUES        MDF4_DL_FLAG_TIME_VALUES
#define MDF4_LD_FLAG_ANGLE_VALUES       MDF4_DL_FLAG_ANGLE_VALUES
#define MDF4_LD_FLAG_DISTANCE_VALUES    MDF4_DL_FLAG_DISTANCE_VALUES
#define MDF4_LD_FLAG_INVALID_DATA_LIST  (1u<<31)  /* If set, the ld_inval_data list is present, otherwise it is missing */

typedef struct ldblock_links
{
  LINK64       ld_ld_next;           /* pointer to (next) list data block (LDBLOCK) (can be NIL)
                                     */
  LINK64       ld_data[1];           /* variable length list of links to DVBLOCKs or RVBLOCKs (mix not allowed!).
                                     Note: each of the blocks may be replaced by a DZBLOCK for the respective block type.
                                     length of list must be equal to ld_count
                                     */
  //LINK64       ld_inval_data[1];   /* variable length list of links to DIBLOCKs or RIBLOCKs (mix not allowed!). */
                                     /* only present if MDF4_LD_FLAG_INVALID_DATA_LIST is set in ld_flags */
                                     /* Note: each of the blocks may be replaced by a DZBLOCK for the respective block type.
                                     single blocks can be NIL if no invalidation bit is set (also for performance improvement, avoids unnecessary check)
                                     length of list must be equal to ld_count
                                     */
} LDBLOCK_LINKS;


typedef struct ldblock_data
{
  DWORD        ld_flags;             /*4*/  /* flags are bit combination of [MDF4_LD_FLAG_xxx] */
  DWORD        ld_count;             /*4*/  /* number of referenced data blocks

                                            If the MDF4_LD_FLAG_EQUAL_SAMPLE_COUNT is set, then ld_count must be equal for each LDBLOCK in the linked list
                                            except for the last one. For the last LDBLOCK (i.e. ld_ld_next = NIL) in this case the value of ld_count
                                            can be less than or equal to ld_count of the previous LDBLOCK.
                                            */
  QWORD        ld_equal_sample_count;/*8*/ /* Every block in ld_data list has a data section with a length equal to ld_equal_sample_count.
                                           This must be true for each LDBLOCK within the linked list, and has only one exception:
                                           the very last block (ld_data[ld_count-1] of last LDBLOCK in linked list) may have a data section
                                           with a different length which must be less than or equal to ld_equal_sample_count.

                                           only present if MDF4_LD_FLAG_EQUAL_SAMPLE_COUNT is set in ld_flags
                                           */

 /* QWORD        ld_sample_offset[1]; */ /*8*/  /* variable length list of offset values in samples. Length of list must be equal to ld_count.
                                             only present if MDF4_LD_FLAG_EQUAL_SAMPLE_COUNT is NOT set in ld_flags

                                             ld_offset[i] is the accumulated data offset for the referenced block that starts at ld_data[i].
                                             If all BLOCK_DATA parts of the blocks in ld_data list of the previous LDBLOCK and of the current LDBLOCK are concatenated,
                                             ld_offset[i] thus is the offset where the BLOCK_DATA parts of the block ld_data[i] would start.

                                             n - 1
                                             __
                                             ld_offset[n] = >_  (ld_data[i].length - sizeof(BLOCK_HEADER) - ld_data[i].link_count * sizeof(LINK64))

                                             i = 0

                                             (i is the running index over all referenced blocks of all LDBLOCKs in the linked list up to the current LDBLOCK)

                                             Hence, ld_offset[0] of the very first LDBLOCK in the linked list must always be zero.
                                             */

  /* INT64/REAL    ld_time_values[1]; */ /*8*/  /* variable length list of raw time values
                                                Length of list must be equal to ld_count.
                                                only present if MDF4_LD_FLAG_TIME_VALUES is set in ld_flags

                                                ld_time_values[i] is the (raw) time value of the first record in the respective data block.
                                                The value must be interpreted either as INT64 or REAL, depending on data type of time master channel.
                                                */

  /* INT64/REAL     ld_angle_values[1]; */ /*8*/  /* variable length list of raw angle values
                                                  Length of list must be equal to ld_count.
                                                  only present if MDF4_LD_FLAG_ANGLE_VALUES is set in ld_flags

                                                  ld_angle_values[i] is the (raw) angle value of the first record in the respective data block.
                                                  The value must be interpreted either as INT64 or REAL, depending on data type of angle master channel.
                                                  */

  /* INT64/REAL     ld_distance_values[1]; */ /*8*/  /* variable length list of raw distance values
                                                     Length of list must be equal to ld_count.
                                                     only present if MDF4_LD_FLAG_DISTANCE_VALUES is set in ld_flags

                                                     ld_distance_values[i] is the (raw) distance value of the first record in the respective data block.
                                                     The value must be interpreted either as INT64 or REAL, depending on data type of distance master channel.
                                                     */


} LDBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* DZBLOCK                                                                  */
/* compressed (zipped) data block (since MDF 4.1)                           */
/* DT, SD or RD block compressed by gzip                                    */

#define MDF4_DZ_ID          GENERATE_ID('D', 'Z')   /* Identification */
#define MDF4_DZ_MIN_LENGTH  (sizeof(BLOCK_HEADER) + sizeof(DZBLOCK_DATA))
#define MDF4_DZ_LINK_COUNT  0

#define MDF4_ZIP_TYPE_DEFLATE          0     /* Deflate algorithm */
#define MDF4_ZIP_TYPE_TRANS_DEFLATE    1     /* Transposed data block + deflate algorithm */
#define MDF4_ZIP_TYPE_NONE             0xFF

#define MDF4_BLOCK_TYPE_DT             ((WORD)('D') + 0x100*('T'))     /* ##DT block */
#define MDF4_BLOCK_TYPE_SD             ((WORD)('S') + 0x100*('D'))     /* ##SD block */
#define MDF4_BLOCK_TYPE_RD             ((WORD)('R') + 0x100*('D'))     /* ##RD block */
#define MDF4_BLOCK_TYPE_DV             ((WORD)('D') + 0x100*('V'))     /* ##DV block */
#define MDF4_BLOCK_TYPE_DI             ((WORD)('D') + 0x100*('I'))     /* ##DI block */
#define MDF4_BLOCK_TYPE_RV             ((WORD)('R') + 0x100*('V'))     /* ##RV block */
#define MDF4_BLOCK_TYPE_RI             ((WORD)('R') + 0x100*('I'))     /* ##RI block */

// Data part can be empty!
typedef struct dzblock_data
{

  WORD         dz_org_block_type;      /*2*/  /* Block type of the original (replaced) data block, see [MDF4_BLOCK_TYPE_xxx] */
  BYTE         dz_zip_type;            /*1*/  /* Zip algorithm used to compress the data stored in dz_data, see [MDF4_ZIP_TYPE_xxx] */
  BYTE         dz_reserved;            /*1*/  /* reserved */
  DWORD        dz_zip_parameter;       /*4*/  /* parameter for the respective zip type:
                                                 MDF4_ZIP_TYPE_GZIP: must be 0
                                                 MDF4_ZIP_TYPE_TRANS_GZIP, MDF4_ZIP_TYPE_DELTA_TRANS_GZIP: number of columns of original matrix (usually record length)
                                              */
  QWORD        dz_org_data_length;     /*8*/  /* length of uncompressed data in Bytes (note: can be less than dz_data_length!) */
  QWORD        dz_data_length;         /*8*/  /* length N of compressed data in Bytes as stored in dz_data */

/*  BYTE       dz_data[1];  */         /*N*/  /* variable length of data bytes, length specified dz_data_length */
} DZBLOCK_DATA;


/* ######################################################################## */

/*--------------------------------------------------------------------------*/
/* HLBLOCK                                                                  */
/* header block for data list block (since MDF 4.1)                         */
/* start linked list of DL blocks that can contain DZBLOCKs                 */

#define MDF4_HL_ID              GENERATE_ID('H', 'L')   /* Identification */
#define MDF4_HL_MIN_LENGTH      (sizeof(BLOCK_HEADER) + 2*8)
#define MDF4_HL_MIN_LINK_COUNT  1

#define MDF4_HL_FLAG_EQUAL_LENGTH     MDF4_DL_FLAG_EQUAL_LENGTH
#define MDF4_HL_FLAG_TIME_VALUES      MDF4_DL_FLAG_TIME_VALUES
#define MDF4_HL_FLAG_ANGLE_VALUES     MDF4_DL_FLAG_ANGLE_VALUES
#define MDF4_HL_FLAG_DISTANCE_VALUES  MDF4_DL_FLAG_DISTANCE_VALUES

typedef struct hlblock_links
{
  LINK64       hl_dl_first;          /* pointer to first data list block (DLBLOCK) (cannot be NIL) */

} HLBLOCK_LINKS;


typedef struct hlblock_data
{
  WORD         hl_flags;             /*2*/  /* flags are bit combination of [MDF4_HL_FLAG_xxx]. If there are unknown flags set, the list must be ignored! */
  BYTE         hl_zip_type;          /*1*/  /* Zip algorithm used to compress the data stored in contained data blocks
                                               when using a DZBLOCK instead of original block type, see [MDF4_ZIP_TYPE_xxx] */
  BYTE         hl_reserved[5];       /*5*/  /* reserved */

} HLBLOCK_DATA;

#pragma pack(pop)

